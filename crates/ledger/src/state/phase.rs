//! Phase — the dramatic arc work unit with four-layer constraints.
//!
//! A Phase replaces "chapter" as the unit of creative work.
//! Chapters are derived output (split by word count), not planning units.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::effect::beat::Beat;
use crate::state::constraint::check_assertions;
use crate::state::snapshot::Snapshot;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhaseType {
    #[default]
    Narrative,
    Worldbuilding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintSource {
    Manual,
    SynopsisDerived,
}

// ══════════════════════════════════════════════════════════════════
// Constraints — one per layer
// ══════════════════════════════════════════════════════════════════

/// Worldbuilding entity count requirements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldbuildingCounts {
    #[serde(default)]
    pub characters: Option<u32>,
    #[serde(default)]
    pub locations: Option<u32>,
    #[serde(default)]
    pub factions: Option<u32>,
    #[serde(default)]
    pub secrets: Option<u32>,
}

/// L1 · Ledger: world state consistency
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerConstraints {
    /// Assertions checked after every beat (violation = reject beat)
    #[serde(default)]
    pub invariants: Vec<StateAssertion>,

    /// Assertions that must hold when phase is submitted
    #[serde(default)]
    pub exit_state: Vec<StateAssertion>,

    /// Worldbuilding: minimum entity counts by type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_entities: Option<WorldbuildingCounts>,

    /// Worldbuilding: minimum average relationships per character
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rel_density: Option<f32>,
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

impl StateAssertion {
    /// Validates that the query string matches one of the supported patterns.
    pub fn validate_syntax(&self) -> Result<(), crate::LedgerError> {
        let q = &self.query;
        if q.starts_with("entity_alive(") && q.ends_with(')') {
            return Ok(());
        }
        if q.starts_with("knows(") && q.ends_with(')') {
            return Ok(());
        }
        if q.contains('.') {
            let parts: Vec<&str> = q.splitn(2, '.').collect();
            if parts.len() == 2 {
                let field = parts[1];
                if ["location", "type", "name"].contains(&field) {
                    return Ok(());
                }
                if field.starts_with("has_trait(") && field.ends_with(')') {
                    return Ok(());
                }
                if field.starts_with("rel(") && field.ends_with(')') {
                    return Ok(());
                }
                if field.starts_with("has_item(") && field.ends_with(')') {
                    return Ok(());
                }
            }
        }
        Err(crate::LedgerError::Parse(format!(
            "Invalid StateAssertion syntax: {}",
            q
        )))
    }
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
    pub phase_type: PhaseType,

    #[serde(default)]
    pub order: u32,

    #[serde(default)]
    pub depends_on: Vec<String>,

    #[serde(default)]
    pub synopsis: Option<String>,

    /// Author guidance / notes for this phase (shown in gen output and AI prompts)
    #[serde(default)]
    pub guidance: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint_source: Option<ConstraintSource>,

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
    pub definition_status: DefinitionStatus,
    pub beats: u32,
    pub words: u32,
    pub effects_count: u32,
    #[serde(default)]
    pub blockers: Vec<String>,

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
    #[serde(default)]
    pub required_tags_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowBeat {
    pub beat: u32,
    pub score: u8,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionStatus {
    Missing,
    Partial,
    Derived,
    Explicit,
}

#[derive(Debug, Clone, Default)]
pub struct EvaluatorInput {
    pub annotations_count: u32,
    pub avg_score: Option<f32>,
    pub low_beats: Vec<LowBeat>,
    pub present_tags: Vec<String>,
}

// ══════════════════════════════════════════════════════════════════
// File I/O
// ══════════════════════════════════════════════════════════════════

impl Phase {
    pub fn has_ledger_constraints(&self) -> bool {
        !self.constraints.ledger.invariants.is_empty()
            || !self.constraints.ledger.exit_state.is_empty()
            || self.constraints.ledger.min_entities.is_some()
            || self.constraints.ledger.min_rel_density.is_some()
    }

    pub fn has_resolver_constraints(&self) -> bool {
        !self.constraints.resolver.intents.is_empty()
            || self.constraints.resolver.min_effects.is_some()
            || self.constraints.resolver.min_relationship_changes.is_some()
    }

    pub fn has_executor_constraints(&self) -> bool {
        self.constraints.executor.words.is_some()
            || self.constraints.executor.per_beat.is_some()
            || self.constraints.executor.pov.is_some()
            || self.constraints.executor.tone.is_some()
            || self.constraints.executor.tone_arc.is_some()
            || !self.constraints.executor.writing_plan.is_empty()
    }

    pub fn has_evaluator_constraints(&self) -> bool {
        self.constraints.evaluator.min_avg_score.is_some()
            || self.constraints.evaluator.max_boring_beats.is_some()
            || !self.constraints.evaluator.required_tags.is_empty()
    }

    pub fn has_any_constraints(&self) -> bool {
        self.has_ledger_constraints()
            || self.has_resolver_constraints()
            || self.has_executor_constraints()
            || self.has_evaluator_constraints()
    }

    pub fn definition_status(&self) -> DefinitionStatus {
        let layers = [
            self.has_ledger_constraints(),
            self.has_resolver_constraints(),
            self.has_executor_constraints(),
            self.has_evaluator_constraints(),
        ];
        let defined_layers = layers.into_iter().filter(|defined| *defined).count();
        match defined_layers {
            0 => DefinitionStatus::Missing,
            4 => match self.constraint_source {
                Some(ConstraintSource::SynopsisDerived) => DefinitionStatus::Derived,
                _ => DefinitionStatus::Explicit,
            },
            _ => DefinitionStatus::Partial,
        }
    }

    pub fn evaluate_progress(
        &self,
        snapshot: &Snapshot,
        beats: &[Beat],
        evaluator: &EvaluatorInput,
    ) -> PhaseProgress {
        let definition_status = self.definition_status();
        let mut blockers = Vec::new();

        let (inv_ok, inv_failures) =
            check_assertions(snapshot, &self.constraints.ledger.invariants);
        let (exit_ok, exit_failures) =
            check_assertions(snapshot, &self.constraints.ledger.exit_state);
        // Worldbuilding constraints
        let (wb_ok, wb_failures) = crate::state::constraint::check_worldbuilding(
            snapshot,
            &self.constraints.ledger.min_entities,
            self.constraints.ledger.min_rel_density,
        );

        let ledger_defined = self.has_ledger_constraints();
        let ledger_status = if !ledger_defined {
            blockers.push("L1 未定义：缺少 invariants / exit_state".to_string());
            LayerStatus::NotChecked
        } else if inv_ok && exit_ok && wb_ok {
            LayerStatus::Ok
        } else {
            if !inv_ok {
                blockers.push("L1 未通过：存在 invariant 违反".to_string());
            }
            if !exit_ok {
                blockers.push("L1 未通过：exit_state 尚未满足".to_string());
            }
            for f in &wb_failures {
                blockers.push(format!("L1 未通过：worldbuilding {f}"));
            }
            LayerStatus::Partial
        };

        let total_effects = beats.iter().map(|b| b.effects.len() as u32).sum::<u32>();
        let resolver_defined = self.has_resolver_constraints();
        let min_effects = self.constraints.resolver.min_effects.unwrap_or(0);
        let effects_met = total_effects >= min_effects;
        let intents_pending: Vec<String> = self
            .constraints
            .resolver
            .intents
            .iter()
            .map(|intent| intent.to_string())
            .collect();
        let resolver_status = if !resolver_defined {
            blockers.push("L2 未定义：缺少 drama 约束".to_string());
            LayerStatus::NotChecked
        } else if !intents_pending.is_empty() {
            blockers.push("L2 存在 intents，但当前实现还不能验证它们".to_string());
            LayerStatus::Partial
        } else if effects_met {
            LayerStatus::Ok
        } else {
            blockers.push(format!("L2 未通过：effects {total_effects}/{min_effects}"));
            LayerStatus::Partial
        };

        let total_words = Beat::total_words(beats);
        let beat_count = beats.len() as u32;
        let plan_count = self.constraints.executor.writing_plan.len() as u32;
        let (word_min, word_max) = self.constraints.executor.words.unwrap_or((0, u32::MAX));
        let words_met = self
            .constraints
            .executor
            .words
            .is_none_or(|(min, max)| total_words >= min && total_words <= max);
        let per_beat_met = self.constraints.executor.per_beat.is_none_or(|(min, max)| {
            !beats.is_empty()
                && beats
                    .iter()
                    .all(|beat| beat.word_count >= min && beat.word_count <= max)
        });
        let planned_beats_met = plan_count == 0 || beat_count >= plan_count;
        let soft_executor_constraints = self.constraints.executor.pov.is_some()
            || self.constraints.executor.tone.is_some()
            || self.constraints.executor.tone_arc.is_some();
        let executor_defined = self.has_executor_constraints();
        let beats_remaining = if beat_count < plan_count {
            self.constraints.executor.writing_plan[beat_count as usize..].to_vec()
        } else {
            Vec::new()
        };
        let executor_status = if !executor_defined {
            blockers.push("L3 未定义：缺少 writing 约束".to_string());
            LayerStatus::NotChecked
        } else if soft_executor_constraints {
            blockers.push("L3 包含 POV / tone 软约束，当前实现还不能自动验证".to_string());
            LayerStatus::Partial
        } else if words_met && per_beat_met && planned_beats_met {
            LayerStatus::Ok
        } else {
            if !words_met {
                blockers.push(format!(
                    "L3 未通过：words {total_words}/{word_min}-{word_max}"
                ));
            }
            if !per_beat_met {
                blockers.push("L3 未通过：存在 beat 字数超出 per_beat 范围".to_string());
            }
            if !planned_beats_met {
                blockers.push(format!("L3 未通过：beats {beat_count}/{plan_count}"));
            }
            LayerStatus::InProgress
        };

        let evaluator_defined = self.has_evaluator_constraints();
        let min_avg = self.constraints.evaluator.min_avg_score.unwrap_or(0.0);
        let max_boring = self
            .constraints
            .evaluator
            .max_boring_beats
            .unwrap_or(u32::MAX);
        let required_tags_missing: Vec<String> = self
            .constraints
            .evaluator
            .required_tags
            .iter()
            .filter(|tag| !evaluator.present_tags.iter().any(|present| present == *tag))
            .cloned()
            .collect();
        let low_count = evaluator.low_beats.len() as u32;
        let evaluator_status = if !evaluator_defined {
            blockers.push("L4 未定义：缺少 reader 约束".to_string());
            LayerStatus::NotChecked
        } else if evaluator.annotations_count == 0 {
            blockers.push("L4 未检查：当前 phase 尚无 annotations".to_string());
            LayerStatus::NotChecked
        } else {
            let avg_ok = self
                .constraints
                .evaluator
                .min_avg_score
                .is_none_or(|threshold| evaluator.avg_score.unwrap_or(0.0) >= threshold);
            let boring_ok = low_count <= max_boring;
            let tags_ok = required_tags_missing.is_empty();
            if avg_ok && boring_ok && tags_ok {
                LayerStatus::Ok
            } else {
                if !avg_ok {
                    blockers.push(format!(
                        "L4 未通过：avg_score {:.1}/{min_avg}",
                        evaluator.avg_score.unwrap_or(0.0)
                    ));
                }
                if !boring_ok {
                    blockers.push(format!("L4 未通过：low_beats {low_count}/{max_boring}"));
                }
                if !tags_ok {
                    blockers.push(format!(
                        "L4 未通过：缺少 required_tags {:?}",
                        required_tags_missing
                    ));
                }
                LayerStatus::NeedsRevision
            }
        };

        let complete = matches!(
            definition_status,
            DefinitionStatus::Derived | DefinitionStatus::Explicit
        ) && matches!(ledger_status, LayerStatus::Ok)
            && matches!(resolver_status, LayerStatus::Ok)
            && matches!(executor_status, LayerStatus::Ok)
            && matches!(evaluator_status, LayerStatus::Ok);

        let intents_done = if intents_pending.is_empty() && resolver_defined {
            vec!["effects_budget_met".to_string()]
        } else {
            Vec::new()
        };

        PhaseProgress {
            phase_id: self.id.clone(),
            complete,
            definition_status,
            beats: beat_count,
            words: total_words,
            effects_count: total_effects,
            blockers: blockers
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            ledger: LedgerProgress {
                status: ledger_status,
                invariants_passing: inv_ok,
                exit_state_met: exit_ok,
                exit_state_pending: exit_failures
                    .iter()
                    .chain(inv_failures.iter())
                    .chain(wb_failures.iter())
                    .cloned()
                    .collect(),
            },
            resolver: ResolverProgress {
                status: resolver_status,
                effects: format!("{total_effects}/{min_effects}"),
                intents_done,
                intents_pending,
            },
            executor: ExecutorProgress {
                status: executor_status,
                words: format!("{total_words}/{word_min}-{word_max}"),
                beats: format!(
                    "{beat_count}/{}",
                    if plan_count > 0 {
                        plan_count.to_string()
                    } else {
                        beat_count.to_string()
                    }
                ),
                beats_remaining,
            },
            evaluator: EvaluatorProgress {
                status: evaluator_status,
                avg_score: evaluator.avg_score,
                low_beats: evaluator.low_beats.clone(),
                required_tags_missing,
            },
        }
    }

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

        // Validate constraint syntax globally on load
        for inv in &phase.constraints.ledger.invariants {
            inv.validate_syntax()?;
        }
        for exit in &phase.constraints.ledger.exit_state {
            exit.validate_syntax()?;
        }

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
    use crate::input::entity::{Character, Entity};

    #[test]
    fn phase_serde_minimal() {
        let json = r#"{"id":"setup"}"#;
        let phase: Phase = serde_json::from_str(json).unwrap();
        assert_eq!(phase.id, "setup");
        assert_eq!(phase.order, 0);
        assert!(phase.depends_on.is_empty());
        assert!(phase.synopsis.is_none());
        assert!(phase.constraint_source.is_none());
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
            phase_type: PhaseType::Narrative,
            order: 1,
            depends_on: vec!["prev".into()],
            synopsis: Some("测试 phase".into()),
            guidance: None,
            constraint_source: Some(ConstraintSource::Manual),
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

    fn test_phase() -> Phase {
        Phase {
            id: "test".into(),
            phase_type: PhaseType::Narrative,
            order: 0,
            depends_on: vec![],
            synopsis: Some("基安与诺娃对峙".into()),
            guidance: None,
            constraint_source: Some(ConstraintSource::Manual),
            constraints: PhaseConstraints {
                ledger: LedgerConstraints {
                    invariants: vec![StateAssertion {
                        query: "entity_alive(kian)".into(),
                        expected: "true".into(),
                    }],
                    exit_state: vec![StateAssertion {
                        query: "kian.location".into(),
                        expected: "gate".into(),
                    }],
                    min_entities: None,
                    min_rel_density: None,
                },
                resolver: ResolverConstraints {
                    intents: vec![],
                    min_effects: Some(1),
                    min_relationship_changes: None,
                },
                executor: ExecutorConstraints {
                    words: Some((1, 20)),
                    per_beat: Some((1, 20)),
                    pov: None,
                    tone: None,
                    tone_arc: None,
                    writing_plan: vec![BeatPlan {
                        label: "opening".into(),
                        target_words: Some(5),
                        effects: vec![],
                        guidance: None,
                    }],
                },
                evaluator: EvaluatorConstraints {
                    min_avg_score: Some(3.0),
                    max_boring_beats: Some(0),
                    required_tags: vec!["tension".into()],
                },
            },
        }
    }

    fn test_snapshot() -> Snapshot {
        Snapshot {
            chapter: "test".into(),
            entities: vec![
                Entity::Character(Character {
                    id: "kian".into(),
                    name: Some("基安".into()),
                    location: Some("gate".into()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    intent_targets: vec![],
                    desire_tags: vec![],
                    inventory: vec![],
                    relationships: vec![],
                    goals: vec![],
            tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "nova".into(),
                    name: Some("诺娃".into()),
                    location: Some("gate".into()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    intent_targets: vec![],
                    desire_tags: vec![],
                    inventory: vec![],
                    relationships: vec![],
                    goals: vec![],
            tags: vec![],
                    description: None,
                }),
            ],
            secrets: vec![],
            goal_entities: vec![],
        }
    }

    #[test]
    fn definition_status_detects_partial() {
        let mut phase = test_phase();
        phase.constraints.ledger = LedgerConstraints::default();
        assert_eq!(phase.definition_status(), DefinitionStatus::Partial);
    }

    #[test]
    fn evaluate_progress_requires_annotations() {
        let phase = test_phase();
        let beats = vec![Beat {
            phase: "test".into(),
            seq: 1,
            revises: None,
            revision: 0,
            text: "基安到了门前".into(),
            effects: vec![crate::Op::AddTrait {
                entity: "kian".into(),
                value: "tense".into(),
            }],
            word_count: 6,
            created_by: "ai".into(),
            created_at: String::new(),
            revision_reason: None,
        }];
        let progress =
            phase.evaluate_progress(&test_snapshot(), &beats, &EvaluatorInput::default());
        assert!(!progress.complete);
        assert!(matches!(progress.evaluator.status, LayerStatus::NotChecked));
    }

    #[test]
    fn evaluate_progress_passes_when_all_layers_pass() {
        let phase = test_phase();
        let beats = vec![Beat {
            phase: "test".into(),
            seq: 1,
            revises: None,
            revision: 0,
            text: "基安到了门前".into(),
            effects: vec![crate::Op::AddTrait {
                entity: "kian".into(),
                value: "tense".into(),
            }],
            word_count: 6,
            created_by: "ai".into(),
            created_at: String::new(),
            revision_reason: None,
        }];
        let progress = phase.evaluate_progress(
            &test_snapshot(),
            &beats,
            &EvaluatorInput {
                annotations_count: 1,
                avg_score: Some(4.0),
                low_beats: vec![],
                present_tags: vec!["tension".into()],
            },
        );
        assert!(progress.complete);
        assert!(progress.blockers.is_empty());
    }
}
