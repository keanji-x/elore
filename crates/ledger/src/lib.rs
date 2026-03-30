//! Ledger — Layer 1: Engine / 事实层
//!
//! Maintains the Single Source of Truth for the narrative world.
//! All state changes are expressed through effects, all queries through Datalog reasoning.
//! **Zero token cost** — pure deterministic replay.
//!
//! # Architecture
//!
//! ```text
//! input/     ← Human-editable genesis data (Ground Truth)
//!   entity     Character/Location/Faction JSON schemas
//!   goal       Desire tree YAML schemas
//!   secret     Information disclosure YAML schemas
//!
//! state/     ← Computed world state (Derived, rebuildable)
//!   snapshot   WorldState = fold(genesis, effects)
//!   graph      Entity relationship graph index
//!   reasoning  Datalog reasoning via Nemo
//!
//! effect/    ← Mutation system (Append-only)
//!   op         Effect operation definitions
//!   history    Append-only event log
//!   diff       Snapshot diff + change propagation
//! ```

pub mod effect;
pub mod input;
pub mod state;

mod error;
pub use error::LedgerError;

// ── Convenience re-exports ───────────────────────────────────────

pub use effect::beat::Beat;
pub use effect::history::{History, HistoryEntry};
pub use effect::op::Op;
pub use input::card;
pub use input::entity::{Character, Entity, Faction, Location};
pub use input::goal::{Goal, GoalEntity, GoalStatus};
pub use input::secret::{Secret, SecretsFileOut};
pub use state::graph::WorldGraph;
pub use state::phase::{
    ConstraintSource, DefinitionStatus, EvaluatorInput, Phase, PhaseConstraints, PhaseProgress,
    PhaseStatus, PhaseType, WorldbuildingCounts,
};
pub use state::phase_manager::ProjectState;
pub use state::reasoning::{ReasoningResult, run_reasoning};
pub use state::snapshot::Snapshot;
