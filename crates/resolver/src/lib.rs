//! Resolver — Layer 2: Director / 意图层
//!
//! The Director manages "dramatic intent" — what should happen in each chapter
//! from a storytelling perspective. It **cannot** directly modify output text;
//! it can only influence the narrative by:
//!
//! 1. Declaring dramatic intents (confrontation, reveal, reversal, etc.)
//! 2. Validating that the world snapshot satisfies those intents
//! 3. Constructing prompts that encode both world state and dramatic goals
//!
//! The Director submits effects to Layer 1 (Engine) to achieve its goals —
//! it never bypasses the Engine to modify entities directly.
//!
//! ```text
//! Engine (Snapshot) → Director (Validate + Prompt) → Author (Text)
//! ```

pub mod drama;
pub mod intent;
pub mod prompt;
pub mod protocol;
pub mod validate;

mod error;
pub use error::ResolverError;
