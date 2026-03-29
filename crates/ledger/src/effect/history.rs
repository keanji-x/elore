//! Timeline Engine — event sourcing for narrative state.
//!
//! Entity JSON files are genesis state (never mutated by effects).
//! All chapter-driven changes are tracked in `history.jsonl` as an
//! append-only event log. Current world state is computed by replaying
//! events on top of genesis.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;
use crate::effect::op::Op;
use crate::input::entity::Entity;
use crate::input::goal::GoalEntity;
use crate::input::secret::Secret;

const HISTORY_FILE: &str = "history.jsonl";

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A single entry in the history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub chapter: String,
    pub seq: u32,
    #[serde(flatten)]
    pub effect: Op,
}

/// The full history log — an append-only event stream.
#[derive(Debug, Clone, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

// ══════════════════════════════════════════════════════════════════
// Load / Save
// ══════════════════════════════════════════════════════════════════

impl History {
    /// Load history from `.everlore/history.jsonl`.
    pub fn load(everlore_dir: &Path) -> Self {
        let path = everlore_dir.join(HISTORY_FILE);
        if !path.exists() {
            return Self::default();
        }
        let file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return Self::default(),
        };

        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<HistoryEntry>(trimmed) {
                Ok(entry) => entries.push(entry),
                Err(e) => log::warn!("Skipping malformed history line: {e}"),
            }
        }
        Self { entries }
    }

    /// Append entries to the history file.
    pub fn append(everlore_dir: &Path, entries: &[HistoryEntry]) -> Result<(), LedgerError> {
        let path = everlore_dir.join(HISTORY_FILE);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        for entry in entries {
            let json = serde_json::to_string(entry)?;
            writeln!(file, "{json}")?;
        }
        Ok(())
    }

    // ── Queries ──────────────────────────────────────────────────

    /// Check if a chapter has any effects applied.
    pub fn has_chapter(&self, chapter: &str) -> bool {
        self.entries.iter().any(|e| e.chapter == chapter)
    }

    /// Get all entries for a specific chapter.
    pub fn chapter_entries(&self, chapter: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.chapter == chapter)
            .collect()
    }

    /// Get all unique chapter names in order of first appearance.
    pub fn chapters(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for entry in &self.entries {
            if !seen.contains(&entry.chapter) {
                seen.push(entry.chapter.clone());
            }
        }
        seen
    }

    /// Get the next available sequence number for a chapter.
    pub fn next_seq(&self, chapter: &str) -> u32 {
        self.entries
            .iter()
            .filter(|e| e.chapter == chapter)
            .map(|e| e.seq)
            .max()
            .map(|m| m + 1)
            .unwrap_or(1)
    }

    // ── Replay ───────────────────────────────────────────────────

    /// Replay all history entries onto entities, up to and including `up_to_chapter`.
    /// If `up_to_chapter` is None, replays everything.
    pub fn replay_entities(
        entities: &mut [Entity],
        history: &History,
        up_to_chapter: Option<&str>,
    ) {
        let chapters = history.chapters();
        let replay_chapters: Vec<&String> = if let Some(up_to) = up_to_chapter {
            let mut selected = Vec::new();
            for ch in &chapters {
                selected.push(ch);
                if ch == up_to {
                    break;
                }
            }
            selected
        } else {
            chapters.iter().collect()
        };

        for entry in &history.entries {
            if !replay_chapters.contains(&&entry.chapter) {
                continue;
            }
            for entity in entities.iter_mut() {
                entry.effect.apply_to_entity(entity);
            }
        }
    }

    /// Replay history up to but **excluding** the given chapter.
    /// This is the correct semantic for narrating chapter N:
    /// the world state should reflect the end of chapter N-1.
    pub fn replay_before(entities: &mut [Entity], history: &History, current_chapter: &str) {
        let chapters = history.chapters();
        let replay_chapters: Vec<&String> = chapters
            .iter()
            .take_while(|ch| ch.as_str() != current_chapter)
            .collect();

        for entry in &history.entries {
            if !replay_chapters.contains(&&entry.chapter) {
                continue;
            }
            for entity in entities.iter_mut() {
                entry.effect.apply_to_entity(entity);
            }
        }
    }

    /// Replay secret-related effects.
    pub fn replay_secrets(secrets: &mut [Secret], history: &History, up_to: Option<&str>) {
        let chapters = history.chapters();
        let replay_chapters: Vec<&String> = if let Some(up_to) = up_to {
            let mut selected = Vec::new();
            for ch in &chapters {
                selected.push(ch);
                if ch == up_to {
                    break;
                }
            }
            selected
        } else {
            chapters.iter().collect()
        };

        for entry in &history.entries {
            if !replay_chapters.contains(&&entry.chapter) {
                continue;
            }
            for secret in secrets.iter_mut() {
                entry.effect.apply_to_secret(secret);
            }
        }
    }

    /// Replay goal-related effects.
    pub fn replay_goals(goal_entities: &mut [GoalEntity], history: &History, up_to: Option<&str>) {
        let chapters = history.chapters();
        let replay_chapters: Vec<&String> = if let Some(up_to) = up_to {
            let mut selected = Vec::new();
            for ch in &chapters {
                selected.push(ch);
                if ch == up_to {
                    break;
                }
            }
            selected
        } else {
            chapters.iter().collect()
        };

        for entry in &history.entries {
            if !replay_chapters.contains(&&entry.chapter) {
                continue;
            }
            for ge in goal_entities.iter_mut() {
                entry.effect.apply_to_goal(ge);
            }
        }
    }

    // ── Rollback ─────────────────────────────────────────────────

    /// Remove all entries for a chapter from the log file.
    pub fn rollback(everlore_dir: &Path, chapter: &str) -> Result<usize, LedgerError> {
        let path = everlore_dir.join(HISTORY_FILE);
        if !path.exists() {
            return Ok(0);
        }

        let history = Self::load(everlore_dir);
        let before_count = history.entries.len();
        let remaining: Vec<&HistoryEntry> = history
            .entries
            .iter()
            .filter(|e| e.chapter != chapter)
            .collect();
        let removed = before_count - remaining.len();

        let mut file = fs::File::create(&path)?;
        for entry in &remaining {
            let json = serde_json::to_string(entry)?;
            writeln!(file, "{json}")?;
        }
        Ok(removed)
    }

    // ── Summary ──────────────────────────────────────────────────

    /// Generate a "Previously On…" summary from the chapter before `current`.
    pub fn previous_chapter_summary(history: &History, current_chapter: &str) -> Option<String> {
        let chapters = history.chapters();
        let prev = {
            let mut prev = None;
            for ch in &chapters {
                if ch == current_chapter {
                    break;
                }
                prev = Some(ch.clone());
            }
            prev
        }?;

        let entries: Vec<&HistoryEntry> = history
            .entries
            .iter()
            .filter(|e| e.chapter == prev)
            .collect();

        if entries.is_empty() {
            return None;
        }

        let mut summary = format!("在 {} 中发生了以下变化：\n", prev);
        for entry in &entries {
            summary.push_str(&format!("- {}\n", entry.effect.describe()));
        }
        Some(summary)
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(chapter: &str, seq: u32, op: Op) -> HistoryEntry {
        HistoryEntry {
            chapter: chapter.into(),
            seq,
            effect: op,
        }
    }

    #[test]
    fn chapters_in_order() {
        let history = History {
            entries: vec![
                make_entry(
                    "ch01",
                    1,
                    Op::AddTrait {
                        entity: "a".into(),
                        value: "x".into(),
                    },
                ),
                make_entry(
                    "ch02",
                    1,
                    Op::AddTrait {
                        entity: "a".into(),
                        value: "y".into(),
                    },
                ),
                make_entry(
                    "ch01",
                    2,
                    Op::AddTrait {
                        entity: "a".into(),
                        value: "z".into(),
                    },
                ),
            ],
        };
        assert_eq!(history.chapters(), vec!["ch01", "ch02"]);
    }

    #[test]
    fn next_seq() {
        let history = History {
            entries: vec![
                make_entry(
                    "ch01",
                    1,
                    Op::AddTrait {
                        entity: "a".into(),
                        value: "x".into(),
                    },
                ),
                make_entry(
                    "ch01",
                    2,
                    Op::AddTrait {
                        entity: "a".into(),
                        value: "y".into(),
                    },
                ),
            ],
        };
        assert_eq!(history.next_seq("ch01"), 3);
        assert_eq!(history.next_seq("ch02"), 1);
    }

    #[test]
    fn replay_entities_applies_effects() {
        let mut entities = vec![Entity {
            entity_type: "character".into(),
            id: "kian".into(),
            name: None,
            traits: vec![],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: Some("wasteland".into()),
            relationships: vec![],
            inventory: vec!["刀".into()],
            alignment: None,
            rivals: vec![],
            members: vec![],
            properties: vec![],
            connections: vec![],
            tags: vec![],
        }];

        let history = History {
            entries: vec![
                make_entry(
                    "ch01",
                    1,
                    Op::RemoveItem {
                        entity: "kian".into(),
                        item: "刀".into(),
                    },
                ),
                make_entry(
                    "ch01",
                    2,
                    Op::Move {
                        entity: "kian".into(),
                        location: "oasis".into(),
                    },
                ),
            ],
        };

        History::replay_entities(&mut entities, &history, None);
        assert!(entities[0].inventory.is_empty());
        assert_eq!(entities[0].location.as_deref(), Some("oasis"));
    }

    #[test]
    fn replay_before_excludes_current() {
        let mut entities = vec![Entity {
            entity_type: "character".into(),
            id: "kian".into(),
            name: None,
            traits: vec![],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: Some("wasteland".into()),
            relationships: vec![],
            inventory: vec![],
            alignment: None,
            rivals: vec![],
            members: vec![],
            properties: vec![],
            connections: vec![],
            tags: vec![],
        }];

        let history = History {
            entries: vec![
                make_entry(
                    "ch01",
                    1,
                    Op::AddTrait {
                        entity: "kian".into(),
                        value: "trait_ch01".into(),
                    },
                ),
                make_entry(
                    "ch02",
                    1,
                    Op::AddTrait {
                        entity: "kian".into(),
                        value: "trait_ch02".into(),
                    },
                ),
            ],
        };

        History::replay_before(&mut entities, &history, "ch02");
        assert_eq!(entities[0].traits, vec!["trait_ch01"]);
    }

    #[test]
    fn serialization_roundtrip() {
        let entry = make_entry(
            "ch03",
            1,
            Op::Reveal {
                secret: "oasis_truth".into(),
                to: "kian".into(),
            },
        );
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.chapter, "ch03");
        assert_eq!(parsed.seq, 1);
    }
}
