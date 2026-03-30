//! Input layer — card-based world data.
//!
//! Cards (Markdown + YAML frontmatter) are the source of truth.
//! Effects update cards' YAML frontmatter after build.

pub mod card;
pub mod card_writer;
pub mod entity;
pub mod goal;
pub mod secret;
