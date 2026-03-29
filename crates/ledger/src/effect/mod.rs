//! Effect layer — the mutation system.
//!
//! All state changes are expressed as effects in an append-only log.
//! Entity JSON is never modified by effects — current state is computed
//! by replaying the log on top of genesis.

pub mod beat;
pub mod diff;
pub mod history;
pub mod op;
