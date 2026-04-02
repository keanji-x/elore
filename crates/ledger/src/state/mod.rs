//! State layer — computed world state.
//!
//! State is always derived from `input/ + effect/`.
//! It can be fully rebuilt from scratch at any time.

pub mod constraint;
pub mod content;
pub mod content_constraint;
pub mod fact;
pub mod graph;
pub mod phase;
pub mod phase_manager;
pub mod program;
pub mod reasoning;
pub mod rule;
pub mod snapshot;
