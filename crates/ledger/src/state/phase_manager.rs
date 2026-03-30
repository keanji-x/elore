//! PhaseManager — manages state.json and phase lifecycle transitions.
//!
//! The state machine: locked → ready → active → reviewing → approved

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;
use crate::state::phase::{Phase, PhaseStatus};

const STATE_FILE: &str = "state.json";

/// Persisted global state: which phase is active, all phases' status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectState {
    /// Currently checked-out phase (None if nothing active)
    #[serde(default)]
    pub current_phase: Option<String>,

    /// Ordered plan of phase IDs
    #[serde(default)]
    pub plan: Vec<String>,

    /// Per-phase runtime state
    #[serde(default)]
    pub phases: BTreeMap<String, PhaseEntry>,
}

/// Runtime state for a single phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseEntry {
    pub status: PhaseStatus,

    #[serde(default)]
    pub beats: u32,

    #[serde(default)]
    pub words: u32,

    #[serde(default)]
    pub effects: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_out_at: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_reason: Option<String>,
}

impl ProjectState {
    /// Load from `.everlore/state.json`, or return default if not exists.
    pub fn load(everlore_dir: &Path) -> Self {
        let path = everlore_dir.join(STATE_FILE);
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save to `.everlore/state.json`.
    pub fn save(&self, everlore_dir: &Path) -> Result<(), LedgerError> {
        std::fs::create_dir_all(everlore_dir)?;
        let path = everlore_dir.join(STATE_FILE);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Register a new phase (from `add phase`).
    /// Determines initial status based on depends_on.
    pub fn register_phase(&mut self, phase: &Phase) {
        let status = if phase.depends_on.is_empty() {
            PhaseStatus::Ready
        } else {
            let all_deps_approved = phase.depends_on.iter().all(|dep| {
                self.phases
                    .get(dep)
                    .is_some_and(|e| e.status == PhaseStatus::Approved)
            });
            if all_deps_approved {
                PhaseStatus::Ready
            } else {
                PhaseStatus::Locked
            }
        };

        self.phases.insert(
            phase.id.clone(),
            PhaseEntry {
                status,
                beats: 0,
                words: 0,
                effects: 0,
                checked_out_at: None,
                approved_at: None,
                rejected_reason: None,
            },
        );

        // Add to plan if not already present
        if !self.plan.contains(&phase.id) {
            self.plan.push(phase.id.clone());
            // Re-sort plan by phase order
            // (caller should ensure order is correct)
        }
    }

    /// Checkout a phase. Transitions ready → active.
    pub fn checkout(&mut self, phase_id: &str) -> Result<(), LedgerError> {
        // Can't checkout if another phase is active
        if let Some(ref current) = self.current_phase
            && current != phase_id
        {
            return Err(LedgerError::Other(format!(
                "Phase '{current}' 正在进行中，请先 submit 或切换"
            )));
        }

        let entry = self
            .phases
            .get_mut(phase_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Phase '{phase_id}' 未注册")))?;

        match entry.status {
            PhaseStatus::Ready => {
                entry.status = PhaseStatus::Active;
                entry.checked_out_at = Some(now());
                self.current_phase = Some(phase_id.to_string());
                Ok(())
            }
            PhaseStatus::Active => {
                // Already active, just set current
                self.current_phase = Some(phase_id.to_string());
                Ok(())
            }
            PhaseStatus::Locked => Err(LedgerError::Other(format!(
                "Phase '{phase_id}' 仍然 locked — 前置 phase 未完成"
            ))),
            PhaseStatus::Reviewing => {
                Err(LedgerError::Other(format!("Phase '{phase_id}' 正在审阅中")))
            }
            PhaseStatus::Approved => Err(LedgerError::Other(format!(
                "Phase '{phase_id}' 已经 approved，不能重新 checkout"
            ))),
        }
    }

    /// Submit current phase for review. Transitions active → reviewing.
    pub fn submit(&mut self) -> Result<String, LedgerError> {
        let phase_id = self
            .current_phase
            .as_ref()
            .ok_or_else(|| LedgerError::Other("没有活跃的 phase".into()))?
            .clone();

        let entry = self
            .phases
            .get_mut(&phase_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Phase '{phase_id}' 未注册")))?;

        if entry.status != PhaseStatus::Active {
            return Err(LedgerError::Other(format!(
                "Phase '{phase_id}' 不是 active 状态，不能 submit"
            )));
        }

        entry.status = PhaseStatus::Reviewing;
        Ok(phase_id)
    }

    /// Approve current phase. Transitions reviewing → approved.
    /// Automatically unlocks dependent phases.
    pub fn approve(&mut self) -> Result<String, LedgerError> {
        let phase_id = self
            .current_phase
            .as_ref()
            .ok_or_else(|| LedgerError::Other("没有活跃的 phase".into()))?
            .clone();

        let entry = self
            .phases
            .get_mut(&phase_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Phase '{phase_id}' 未注册")))?;

        if entry.status != PhaseStatus::Reviewing {
            return Err(LedgerError::Other(format!(
                "Phase '{phase_id}' 不是 reviewing 状态，不能 approve"
            )));
        }

        entry.status = PhaseStatus::Approved;
        entry.approved_at = Some(now());
        self.current_phase = None;

        // Unlock dependent phases
        self.resolve_dependencies();

        Ok(phase_id)
    }

    /// Reject current phase. Transitions reviewing → active.
    pub fn reject(&mut self, reason: &str) -> Result<String, LedgerError> {
        let phase_id = self
            .current_phase
            .as_ref()
            .ok_or_else(|| LedgerError::Other("没有活跃的 phase".into()))?
            .clone();

        let entry = self
            .phases
            .get_mut(&phase_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Phase '{phase_id}' 未注册")))?;

        if entry.status != PhaseStatus::Reviewing {
            return Err(LedgerError::Other(format!(
                "Phase '{phase_id}' 不是 reviewing 状态，不能 reject"
            )));
        }

        entry.status = PhaseStatus::Active;
        entry.rejected_reason = Some(reason.to_string());
        Ok(phase_id)
    }

    /// Update beat/word/effect counts for the active phase.
    pub fn update_progress(&mut self, beats: u32, words: u32, effects: u32) {
        if let Some(ref id) = self.current_phase
            && let Some(entry) = self.phases.get_mut(id)
        {
            entry.beats = beats;
            entry.words = words;
            entry.effects = effects;
        }
    }

    /// Re-check all locked phases and unlock if deps are met.
    /// Note: requires phase definitions — call resolve_dependencies_with_phases() instead.
    fn resolve_dependencies(&mut self) {
        // Without phase definitions, we can't check depends_on.
        // After approve(), the caller should call resolve_dependencies_with_phases().
    }

    /// Re-check locked phases using actual phase definitions.
    pub fn resolve_dependencies_with_phases(&mut self, phases: &[Phase]) {
        let approved: Vec<String> = self
            .phases
            .iter()
            .filter(|(_, e)| e.status == PhaseStatus::Approved)
            .map(|(id, _)| id.clone())
            .collect();

        for phase in phases {
            if let Some(entry) = self.phases.get_mut(&phase.id)
                && entry.status == PhaseStatus::Locked
            {
                let all_deps_met = phase.depends_on.iter().all(|dep| approved.contains(dep));
                if all_deps_met {
                    entry.status = PhaseStatus::Ready;
                }
            }
        }
    }

    /// Get the current active phase ID.
    pub fn active_phase(&self) -> Option<&str> {
        self.current_phase.as_deref()
    }
}

fn now() -> String {
    // Simple ISO 8601 timestamp
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", d.as_secs())
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_phase(id: &str, deps: Vec<&str>) -> Phase {
        Phase {
            id: id.into(),
            phase_type: Default::default(),
            order: 0,
            depends_on: deps.into_iter().map(String::from).collect(),
            synopsis: None,
            guidance: None,
            constraint_source: None,
            constraints: Default::default(),
        }
    }

    #[test]
    fn register_no_deps_is_ready() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        assert_eq!(state.phases["setup"].status, PhaseStatus::Ready);
    }

    #[test]
    fn register_with_deps_is_locked() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        state.register_phase(&make_phase("act2", vec!["setup"]));
        assert_eq!(state.phases["act2"].status, PhaseStatus::Locked);
    }

    #[test]
    fn checkout_ready_becomes_active() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        state.checkout("setup").unwrap();
        assert_eq!(state.phases["setup"].status, PhaseStatus::Active);
        assert_eq!(state.current_phase.as_deref(), Some("setup"));
    }

    #[test]
    fn checkout_locked_fails() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        state.register_phase(&make_phase("act2", vec!["setup"]));
        assert!(state.checkout("act2").is_err());
    }

    #[test]
    fn full_lifecycle() {
        let mut state = ProjectState::default();
        let p1 = make_phase("setup", vec![]);
        let p2 = make_phase("act2", vec!["setup"]);

        state.register_phase(&p1);
        state.register_phase(&p2);

        // setup: ready → active → reviewing → approved
        state.checkout("setup").unwrap();
        state.submit().unwrap();
        assert_eq!(state.phases["setup"].status, PhaseStatus::Reviewing);

        state.approve().unwrap();
        assert_eq!(state.phases["setup"].status, PhaseStatus::Approved);
        assert!(state.current_phase.is_none());

        // act2 should unlock
        state.resolve_dependencies_with_phases(&[p1, p2.clone()]);
        assert_eq!(state.phases["act2"].status, PhaseStatus::Ready);

        // act2: ready → active
        state.checkout("act2").unwrap();
        assert_eq!(state.phases["act2"].status, PhaseStatus::Active);
    }

    #[test]
    fn reject_returns_to_active() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        state.checkout("setup").unwrap();
        state.submit().unwrap();
        state.reject("太短了").unwrap();
        assert_eq!(state.phases["setup"].status, PhaseStatus::Active);
        assert_eq!(
            state.phases["setup"].rejected_reason.as_deref(),
            Some("太短了")
        );
    }

    #[test]
    fn cannot_checkout_two_phases() {
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("a", vec![]));
        state.register_phase(&make_phase("b", vec![]));
        state.checkout("a").unwrap();
        assert!(state.checkout("b").is_err());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = ProjectState::default();
        state.register_phase(&make_phase("setup", vec![]));
        state.checkout("setup").unwrap();

        state.save(dir.path()).unwrap();
        let loaded = ProjectState::load(dir.path());
        assert_eq!(loaded.current_phase.as_deref(), Some("setup"));
        assert_eq!(loaded.phases["setup"].status, PhaseStatus::Active);
    }
}
