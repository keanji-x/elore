//! Content node constraints — flat, node-level only.
//!
//! Unlike `PhaseConstraints` (which has beat-level fields like `per_beat`,
//! `writing_plan`, `min_avg_score`), `ContentConstraints` only contains
//! fields that apply to a single content node.

use serde::{Deserialize, Serialize};

use super::phase::{StateAssertion, WorldbuildingCounts};

/// Constraints for a single content node.
///
/// Flat structure — no L1/L2/L3 wrapper layers.
/// Card YAML maps directly: `constraints.exit_state`, `constraints.words`, etc.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentConstraints {
    // ── L1: world state correctness ─────────────────────────────

    /// State that must hold when this node is committed
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_state: Vec<StateAssertion>,

    /// Invariants that must hold (checked during commit)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<StateAssertion>,

    /// Worldbuilding: minimum entity counts by type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_entities: Option<WorldbuildingCounts>,

    /// Worldbuilding: minimum average relationships per character
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rel_density: Option<f32>,

    // ── L2: minimum effect budget ───────────────────────────────

    /// Minimum number of effects this node must produce
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_effects: Option<u32>,

    /// Minimum relationship changes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_relationship_changes: Option<u32>,

    // ── L3: output control ──────────────────────────────────────

    /// (min, max) word count for this node's text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<(u32, u32)>,

    /// POV character
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pov: Option<String>,

    /// Tone descriptor
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,

    /// Minimum number of POV drafts required before commit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_povs: Option<u32>,
}
