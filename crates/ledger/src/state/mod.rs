//! State layer — computed world state.
//!
//! State is always derived from `input/ + effect/`.
//! It can be fully rebuilt from scratch at any time.

pub mod constraint;
pub mod graph;
pub mod phase;
pub mod phase_manager;
pub mod reasoning;
pub mod snapshot;
