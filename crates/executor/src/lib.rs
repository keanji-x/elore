//! Executor — Layer 3: Author / 执行层
//!
//! The Author takes a prompt from the Director and produces:
//! 1. Narrative text (the actual story content)
//! 2. Annotated effects (what state changes occurred)
//!
//! The Author is LLM-powered — this crate defines the interface
//! and handles text ↔ effect synchronization.

pub mod extract;
pub mod writer;

mod error;
pub use error::ExecutorError;
