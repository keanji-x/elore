//! Phase — the dramatic arc work unit with four-layer constraints.
//!
//! A Phase replaces "chapter" as the unit of creative work.
//! Chapters are derived output (split by word count), not planning units.

use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════
// Phase status
// ══════════════════════════════════════════════════════════════════

/// Phase lifecycle state machine:
/// locked → ready → active → reviewing → approved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PhaseStatus {
    /// depends_on phases not yet approved
    Locked,
    /// Ready to checkout (all deps approved)
    #[default]
    Ready,
    /// Currently being worked on
    Active,
    /// Submitted for review (all constraints met)
    Reviewing,
    /// Approved and permanently locked
    Approved,
}

// ══════════════════════════════════════════════════════════════════
// Constraints — one per layer
// ══════════════════════════════════════════════════════════════════

/// L1 · Ledger: world state consistency
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerConstraints {
    /// Assertions checked after every beat (violation = reject beat)
    #[serde(default)]
    pub invariants: Vec<StateAssertion>,

    /// Assertions that must hold when phase is submitted
    #[serde(default)]
    pub exit_state: Vec<StateAssertion>,
}

/// A simple state assertion in the form "entity.field op value"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateAssertion {
    /// e.g. "kian.location", "kian.knows", "entity_alive(kian)"
    pub query: String,
    /// Expected value. For existence checks, use "true"/"false"
    #[serde(default = "default_expected")]
    pub expected: String,
}

fn default_expected() -> String {
    "true".to_string()
}

/// L2 · Resolver: dramatic arc progression
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolverConstraints {
    /// Dramatic intents that must be satisfied (reuses existing DramaticIntent)
    #[serde(default)]
    pub intents: Vec<serde_json::Value>, // Serialized DramaticIntent — avoids circular dep

    /// Minimum number of effects in this phase
    #[serde(default)]
    pub min_effects: Option<u32>,

    /// Minimum relationship changes
    #[serde(default)]
    pub min_relationship_changes: Option<u32>,
}

/// L3 · Executor: text output control
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutorConstraints {
    /// (min, max) word count for the entire phase
    #[serde(default)]
    pub words: Option<(u32, u32)>,

    /// (min, max) word count per beat
    #[serde(default)]
    pub per_beat: Option<(u32, u32)>,

    /// POV character
    #[serde(default)]
    pub pov: Option<String>,

    /// Tone
    #[serde(default)]
    pub tone: Option<String>,

    /// Tone arc e.g. "好奇 → 恐惧 → 震撼"
    #[serde(default)]
    pub tone_arc: Option<String>,

    /// Planned beats with guidance
    #[serde(default)]
    pub writing_plan: Vec<BeatPlan>,
}

/// A planned beat within a phase's writing plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatPlan {
    pub label: String,
    #[serde(default)]
    pub target_words: Option<u32>,
    #[serde(default)]
    pub effects: Vec<String>,
    #[serde(default)]
    pub guidance: Option<String>,
}

/// L4 · Evaluator: subjective quality
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluatorConstraints {
    /// Minimum average score across all beats (1-5 scale)
    #[serde(default)]
    pub min_avg_score: Option<f32>,

    /// Maximum number of beats with score ≤ 2
    #[serde(default)]
    pub max_boring_beats: Option<u32>,

    /// Tags that must appear on at least one beat
    #[serde(default)]
    pub required_tags: Vec<String>,
}

/// Aggregate four-layer constraints
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhaseConstraints {
    #[serde(default)]
    pub ledger: LedgerConstraints,
    #[serde(default)]
    pub resolver: ResolverConstraints,
    #[serde(default)]
    pub executor: ExecutorConstraints,
    #[serde(default)]
    pub evaluator: EvaluatorConstraints,
}

// ══════════════════════════════════════════════════════════════════
// Phase definition
// ══════════════════════════════════════════════════════════════════

/// A complete Phase definition, stored as `phases/{id}.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub id: String,

    #[serde(default)]
    pub order: u32,

    #[serde(default)]
    pub depends_on: Vec<String>,

    #[serde(default)]
    pub synopsis: Option<String>,

    /// Author guidance / notes for this phase (shown in gen output and AI prompts)
    #[serde(default)]
    pub guidance: Option<String>,

    #[serde(default)]
    pub constraints: PhaseConstraints,
}

// ══════════════════════════════════════════════════════════════════
// Progress tracking (runtime, not persisted in phase definition)
// ══════════════════════════════════════════════════════════════════

/// Per-layer status report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerStatus {
    Ok,
    Partial,
    InProgress,
    NeedsRevision,
    NotChecked,
}

/// Four-layer progress snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseProgress {
    pub phase_id: String,
    pub complete: bool,
    pub beats: u32,
    pub words: u32,
    pub effects_count: u32,

    pub ledger: LedgerProgress,
    pub resolver: ResolverProgress,
    pub executor: ExecutorProgress,
    pub evaluator: EvaluatorProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerProgress {
    pub status: LayerStatus,
    pub invariants_passing: bool,
    pub exit_state_met: bool,
    #[serde(default)]
    pub exit_state_pending: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverProgress {
    pub status: LayerStatus,
    pub effects: String, // "5/8"
    #[serde(default)]
    pub intents_done: Vec<String>,
    #[serde(default)]
    pub intents_pending: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorProgress {
    pub status: LayerStatus,
    pub words: String, // "3500/5000-8000"
    pub beats: String, // "2/3"
    #[serde(default)]
    pub beats_remaining: Vec<BeatPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorProgress {
    pub status: LayerStatus,
    #[serde(default)]
    pub avg_score: Option<f32>,
    #[serde(default)]
    pub low_beats: Vec<LowBeat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowBeat {
    pub beat: u32,
    pub score: u8,
    #[serde(default)]
    pub reason: Option<String>,
}

// ══════════════════════════════════════════════════════════════════
// File I/O
// ══════════════════════════════════════════════════════════════════

impl Phase {
    /// Load a phase definition from `phases/{id}.yaml`
    pub fn load(phases_dir: &std::path::Path, id: &str) -> Result<Self, crate::LedgerError> {
        let path = phases_dir.join(format!("{id}.yaml"));
        if !path.exists() {
            return Err(crate::LedgerError::NotFound(format!(
                "Phase '{id}' not found"
            )));
        }
        let content = std::fs::read_to_string(&path)?;
        let phase: Phase = serde_yaml::from_str(&content)
            .map_err(|e| crate::LedgerError::Parse(format!("Phase '{id}': {e}")))?;
        Ok(phase)
    }

    /// Save phase definition to `phases/{id}.yaml`
    pub fn save(&self, phases_dir: &std::path::Path) -> Result<(), crate::LedgerError> {
        std::fs::create_dir_all(phases_dir)?;
        let path = phases_dir.join(format!("{}.yaml", self.id));
        let content = serde_yaml::to_string(self)
            .map_err(|e| crate::LedgerError::Parse(format!("Phase serialize: {e}")))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// List all phase IDs in the phases directory
    pub fn list(phases_dir: &std::path::Path) -> Vec<String> {
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(phases_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml" || e == "yml")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                {
                    ids.push(stem.to_string());
                }
            }
        }
        ids.sort();
        ids
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_serde_minimal() {
        let json = r#"{"id":"setup"}"#;
        let phase: Phase = serde_json::from_str(json).unwrap();
        assert_eq!(phase.id, "setup");
        assert_eq!(phase.order, 0);
        assert!(phase.depends_on.is_empty());
        assert!(phase.synopsis.is_none());
        // All constraints default to empty
        assert!(phase.constraints.ledger.invariants.is_empty());
        assert!(phase.constraints.resolver.intents.is_empty());
        assert!(phase.constraints.executor.words.is_none());
        assert!(phase.constraints.evaluator.min_avg_score.is_none());
    }

    #[test]
    fn phase_serde_full() {
        let json = r#"{
            "id": "confrontation",
            "order": 2,
            "depends_on": ["setup"],
            "synopsis": "基安与诺娃对峙",
            "constraints": {
                "ledger": {
                    "invariants": [
                        {"query": "entity_alive(kian)"},
                        {"query": "entity_alive(nova)"}
                    ],
                    "exit_state": [
                        {"query": "kian.location", "expected": "oasis_gate"}
                    ]
                },
                "resolver": {
                    "min_effects": 8
                },
                "executor": {
                    "words": [5000, 8000],
                    "pov": "kian",
                    "tone": "紧张",
                    "writing_plan": [
                        {"label": "沙暴穿越", "target_words": 1500, "guidance": "感官细节"}
                    ]
                },
                "evaluator": {
                    "min_avg_score": 3.0,
                    "max_boring_beats": 0,
                    "required_tags": ["twist"]
                }
            }
        }"#;

        let phase: Phase = serde_json::from_str(json).unwrap();
        assert_eq!(phase.id, "confrontation");
        assert_eq!(phase.depends_on, vec!["setup"]);
        assert_eq!(phase.constraints.ledger.invariants.len(), 2);
        assert_eq!(phase.constraints.ledger.exit_state.len(), 1);
        assert_eq!(phase.constraints.resolver.min_effects, Some(8));
        assert_eq!(phase.constraints.executor.words, Some((5000, 8000)));
        assert_eq!(phase.constraints.executor.writing_plan.len(), 1);
        assert_eq!(phase.constraints.evaluator.required_tags, vec!["twist"]);
    }

    #[test]
    fn phase_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let phase = Phase {
            id: "test_phase".into(),
            order: 1,
            depends_on: vec!["prev".into()],
            synopsis: Some("测试 phase".into()),
            guidance: None,
            constraints: PhaseConstraints::default(),
        };

        phase.save(dir.path()).unwrap();
        let loaded = Phase::load(dir.path(), "test_phase").unwrap();
        assert_eq!(loaded.id, "test_phase");
        assert_eq!(loaded.depends_on, vec!["prev"]);
        assert_eq!(loaded.synopsis.as_deref(), Some("测试 phase"));
    }

    #[test]
    fn phase_status_serde() {
        let s = PhaseStatus::Active;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: PhaseStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PhaseStatus::Active);
    }

    #[test]
    fn state_assertion_defaults() {
        let json = r#"{"query": "entity_alive(kian)"}"#;
        let a: StateAssertion = serde_json::from_str(json).unwrap();
        assert_eq!(a.expected, "true");
    }
}
