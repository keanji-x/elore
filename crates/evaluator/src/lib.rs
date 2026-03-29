//! Evaluator — Layer 4: Reader / 观测层
//!
//! The Reader observes the final output and provides feedback:
//! 1. Did the text satisfy the Director's intents?
//! 2. Were required effects reflected in the narrative?
//! 3. Consistency score against the Snapshot.
//!
//! This enables the feedback loop:
//! Director → Author → Reader → Director (next iteration).

pub mod annotation;
pub mod audit;
pub mod score;

mod error;
pub use error::EvaluatorError;
