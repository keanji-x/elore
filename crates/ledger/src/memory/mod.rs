//! Memory — hypergraph-based experiential memory system.
//!
//! Combines two retrieval paradigms:
//!
//! - **System 1 (Intuitive)**: Embedding similarity + multi-signal recall scoring.
//!   Fast, fuzzy, associative — "what would this character think of right now?"
//!
//! - **System 2 (Logical)**: Datalog reasoning over memory facts.
//!   Precise, deterministic — "what does this character *actually* know?"
//!
//! The intersection gives narrative-accurate "intuitive" memory:
//! `recall(char, context) = vector_search(context) ∩ perceives(char, *)`
//!
//! # Architecture
//!
//! ```text
//! Op[] (from content node effects)
//!   ↓ edge::MemoryEdge::from_ops()
//! MemoryEdge (hyperedge connecting participants, locations, secrets, objects)
//!   ↓ perceive::auto_perceive()
//! Perception[] (who knows what, via what mode)
//!   ↓                          ↓
//! embed → MemoryIndex       fact → FactSet → Datalog
//!   ↓                          ↓
//! recall::recall_top_k()    reasoning engine
//!   ↓                          ↓
//! "what they'd recall"     "what they logically know"
//!          ↘               ↙
//!         final context output
//! ```
//!
//! # Zero-cost by default
//!
//! Memory edges and perceptions are generated deterministically from effects
//! during `elore build`. The embedding layer is optional — if no model is
//! available, the system falls back to Datalog-only retrieval.

pub mod edge;
pub mod embed;
pub mod fact;
pub mod harness;
pub mod hypergraph;
pub mod perceive;
pub mod recall;
