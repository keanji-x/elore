//! World snapshot — frozen world state at a chapter boundary.
//!
//! `Snapshot = fold(genesis, effects_up_to_chapter)`
//!
//! A snapshot captures all entities, secrets, and goals at a specific point
//! in the timeline. It is deterministic: the same genesis + effects always
//! produce the same snapshot.

use std::path::Path;

use crate::effect::history::History;
use crate::input::entity;
use crate::input::goal;
use crate::input::secret;
use crate::input::entity::Entity;
use crate::input::goal::GoalEntity;
use crate::input::secret::Secret;
use crate::LedgerError;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A frozen world state at a chapter boundary.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub chapter: String,
    pub entities: Vec<Entity>,
    pub secrets: Vec<Secret>,
    pub goal_entities: Vec<GoalEntity>,
}

// ══════════════════════════════════════════════════════════════════
// Build
// ══════════════════════════════════════════════════════════════════

impl Snapshot {
    /// Build a snapshot reflecting the world state at the END of a chapter.
    /// This includes all effects up to and including that chapter.
    pub fn build(
        chapter: &str,
        entities_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        // 1. Load genesis data
        let mut entities = entity::load_entities(entities_dir)?;
        let mut secrets = secret::load_secrets(entities_dir)?;
        let mut goals = goal::load_goal_entities(entities_dir)?;

        // 2. Load full history and replay up to this chapter
        let history = History::load(everlore_dir);
        History::replay_entities(&mut entities, &history, Some(chapter));
        History::replay_secrets(&mut secrets, &history, Some(chapter));
        History::replay_goals(&mut goals, &history, Some(chapter));

        Ok(Self {
            chapter: chapter.to_string(),
            entities,
            secrets,
            goal_entities: goals,
        })
    }

    /// Build a snapshot reflecting the world state BEFORE a chapter.
    /// This is the correct state for narrating chapter N:
    /// the world should reflect the end of chapter N-1.
    pub fn build_before(
        chapter: &str,
        entities_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        let mut entities = entity::load_entities(entities_dir)?;
        let mut secrets = secret::load_secrets(entities_dir)?;
        let mut goals = goal::load_goal_entities(entities_dir)?;

        let history = History::load(everlore_dir);
        History::replay_before(&mut entities, &history, chapter);
        // replay_before for secrets and goals: replay all chapters before current
        let chapters = history.chapters();
        let prev_chapter = chapters
            .iter()
            .take_while(|ch| ch.as_str() != chapter)
            .last()
            .cloned();

        if let Some(prev) = prev_chapter {
            History::replay_secrets(&mut secrets, &history, Some(&prev));
            History::replay_goals(&mut goals, &history, Some(&prev));
        }

        Ok(Self {
            chapter: chapter.to_string(),
            entities,
            secrets,
            goal_entities: goals,
        })
    }

    /// Build from pre-loaded data (for testing or when data is already in memory).
    pub fn from_parts(
        chapter: &str,
        entities: Vec<Entity>,
        secrets: Vec<Secret>,
        goal_entities: Vec<GoalEntity>,
    ) -> Self {
        Self {
            chapter: chapter.to_string(),
            entities,
            secrets,
            goal_entities,
        }
    }

    // ── Queries ──────────────────────────────────────────────────

    /// Find an entity by ID.
    pub fn entity(&self, id: &str) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Find a secret by ID.
    pub fn secret(&self, id: &str) -> Option<&Secret> {
        self.secrets.iter().find(|s| s.id == id)
    }

    /// All characters in the snapshot.
    pub fn characters(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == "character")
            .collect()
    }

    /// All locations in the snapshot.
    pub fn locations(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == "location")
            .collect()
    }

    /// All factions in the snapshot.
    pub fn factions(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == "faction")
            .collect()
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts_creates_snapshot() {
        let snap = Snapshot::from_parts(
            "ch01",
            vec![Entity {
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
            }],
            vec![],
            vec![],
        );
        assert_eq!(snap.chapter, "ch01");
        assert!(snap.entity("kian").is_some());
        assert!(snap.entity("nova").is_none());
        assert_eq!(snap.characters().len(), 1);
    }
}
