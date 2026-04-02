//! World snapshot — frozen world state at a point in the narrative.
//!
//! `Snapshot = fold(genesis, effects_up_to_position)`
//!
//! A snapshot captures all entities, secrets, and goals at a specific point
//! in the timeline. It is deterministic: the same genesis + effects always
//! produce the same snapshot.
//!
//! In the content tree model, the snapshot is a **cursor** — it has a position
//! in the tree and shows the world state at that position.

use std::collections::BTreeMap;
use std::path::Path;

use crate::LedgerError;
use crate::effect::history::History;
use crate::input::entity;
use crate::input::entity::Entity;
use crate::input::goal;
use crate::input::goal::GoalEntity;
use crate::input::secret;
use crate::input::secret::Secret;
use crate::state::content::{Content, ContentTree};

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A frozen world state at a point in the narrative.
///
/// In the content tree model, this is the **cursor** — positioned at a
/// content node, showing the world state the agent sees.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Position in the narrative (content id or chapter name).
    pub position: String,
    pub entities: Vec<Entity>,
    pub secrets: Vec<Secret>,
    pub goal_entities: Vec<GoalEntity>,
}

// ══════════════════════════════════════════════════════════════════
// Build (legacy chapter-based)
// ══════════════════════════════════════════════════════════════════

impl Snapshot {
    /// Build a snapshot reflecting the world state at the END of a chapter.
    /// This includes all effects up to and including that chapter.
    pub fn build(
        chapter: &str,
        entities_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        let mut entities = entity::load_entities(entities_dir)?;
        let mut secrets = secret::load_secrets(everlore_dir)?;
        let history = History::load(everlore_dir);
        History::replay_entities(&mut entities, &history, Some(chapter));
        History::replay_secrets(&mut secrets, &history, Some(chapter));

        let mut goals = goal::extract_goal_entities(&entities);
        if let Ok(yaml_goals) = goal::load_goal_entities(entities_dir) {
            for yg in yaml_goals {
                if !goals.iter().any(|g| g.id == yg.id) {
                    goals.push(yg);
                }
            }
        }
        History::replay_goals(&mut goals, &history, Some(chapter));

        Ok(Self {
            position: chapter.to_string(),
            entities,
            secrets,
            goal_entities: goals,
        })
    }

    /// Build a snapshot reflecting the world state BEFORE a chapter.
    pub fn build_before(
        chapter: &str,
        entities_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        let mut entities = entity::load_entities(entities_dir)?;
        let mut secrets = secret::load_secrets(everlore_dir)?;

        let history = History::load(everlore_dir);
        History::replay_before(&mut entities, &history, chapter);

        let mut goals = goal::extract_goal_entities(&entities);
        if let Ok(yaml_goals) = goal::load_goal_entities(entities_dir) {
            for yg in yaml_goals {
                if !goals.iter().any(|g| g.id == yg.id) {
                    goals.push(yg);
                }
            }
        }

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
            position: chapter.to_string(),
            entities,
            secrets,
            goal_entities: goals,
        })
    }

    /// Build from pre-loaded data (for testing or when data is already in memory).
    pub fn from_parts(
        position: &str,
        entities: Vec<Entity>,
        secrets: Vec<Secret>,
        goal_entities: Vec<GoalEntity>,
    ) -> Self {
        Self {
            position: position.to_string(),
            entities,
            secrets,
            goal_entities,
        }
    }

    // ── Queries ──────────────────────────────────────────────────

    /// Find an entity by ID.
    pub fn entity(&self, id: &str) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id() == id)
    }

    /// Find a secret by ID.
    pub fn secret(&self, id: &str) -> Option<&Secret> {
        self.secrets.iter().find(|s| s.id == id)
    }

    /// All characters in the snapshot.
    pub fn characters(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.is_character())
            .collect()
    }

    /// All locations in the snapshot.
    pub fn locations(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.is_location())
            .collect()
    }

    /// All factions in the snapshot.
    pub fn factions(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.is_faction())
            .collect()
    }

    // ── Content tree cursor ─────────────────────────────────────

    /// Cursor: world state BEFORE a content node's effects.
    ///
    /// This is what the agent sees when writing this node —
    /// the world as it exists before any of this node's effects are applied.
    pub fn before_content(
        content_id: &str,
        tree: &ContentTree,
        contents: &BTreeMap<String, Content>,
        entities_dir: &Path,
        cards_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        if !tree.nodes.contains_key(content_id) {
            return Err(LedgerError::NotFound(format!(
                "Content '{content_id}' 不在内容树中"
            )));
        }
        let path = tree.replay_path_before(content_id);
        Self::build_from_replay_path(content_id, &path, tree, contents, entities_dir, cards_dir, everlore_dir)
    }

    /// Cursor: world state AFTER a content node's effects.
    ///
    /// This is used for commit validation — checking that the node's
    /// effects produce the expected world state.
    pub fn at_content(
        content_id: &str,
        tree: &ContentTree,
        contents: &BTreeMap<String, Content>,
        entities_dir: &Path,
        cards_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        if !tree.nodes.contains_key(content_id) {
            return Err(LedgerError::NotFound(format!(
                "Content '{content_id}' 不在内容树中"
            )));
        }
        let path = tree.replay_path(content_id);
        Self::build_from_replay_path(content_id, &path, tree, contents, entities_dir, cards_dir, everlore_dir)
    }

    /// Shared replay logic for content tree snapshots.
    ///
    /// Replays ALL nodes in DFS path. For branch nodes, only applies
    /// unclaimed effects (effects not covered by children's inherited_slots).
    /// For leaf nodes, applies all effects. This ensures the snapshot is
    /// always complete regardless of expansion progress.
    fn build_from_replay_path(
        position: &str,
        path: &[String],
        tree: &ContentTree,
        contents: &BTreeMap<String, Content>,
        entities_dir: &Path,
        cards_dir: &Path,
        everlore_dir: &Path,
    ) -> Result<Self, LedgerError> {
        // 1. Load genesis data
        let mut entities = entity::load_entities(entities_dir)?;
        let mut secrets = secret::load_secrets(everlore_dir)?;

        // 2. For each node in path, merge local cards then apply effects
        for node_id in path {
            // Merge local entity cards
            if let Ok(local_entities) =
                crate::input::card::load_local_entity_cards(cards_dir, node_id)
            {
                for (local_entity, _path) in local_entities {
                    if let Some(existing) = entities.iter_mut().find(|e| e.id() == local_entity.id())
                    {
                        *existing = local_entity;
                    } else {
                        entities.push(local_entity);
                    }
                }
            }

            // Merge local secret cards
            if let Ok(local_secrets) =
                crate::input::card::load_local_secret_cards(cards_dir, node_id)
            {
                for local_secret in local_secrets {
                    if let Some(existing) = secrets.iter_mut().find(|s| s.id == local_secret.id) {
                        *existing = local_secret;
                    } else {
                        secrets.push(local_secret);
                    }
                }
            }

            // Compute effective effects for this node
            let effective_ops = Self::effective_effects(node_id, tree, contents);

            for op in &effective_ops {
                for entity in entities.iter_mut() {
                    op.apply_to_entity(entity);
                }
                for secret in secrets.iter_mut() {
                    op.apply_to_secret(secret);
                }
            }
        }

        // 3. Extract goals
        let mut goals = goal::extract_goal_entities(&entities);
        if let Ok(yaml_goals) = goal::load_goal_entities(entities_dir) {
            for yg in yaml_goals {
                if !goals.iter().any(|g| g.id == yg.id) {
                    goals.push(yg);
                }
            }
        }
        // Replay goal effects along path
        for node_id in path {
            let effective_ops = Self::effective_effects(node_id, tree, contents);
            for op in &effective_ops {
                for ge in goals.iter_mut() {
                    op.apply_to_goal(ge);
                }
            }
        }

        Ok(Self {
            position: position.to_string(),
            entities,
            secrets,
            goal_entities: goals,
        })
    }

    /// Compute effective effects for a node during replay.
    ///
    /// - **Leaf**: all effects apply.
    /// - **Branch**: only unclaimed effects apply (effects not in any child's inherited_slots).
    ///
    /// This ensures snapshots are always complete regardless of expansion progress.
    fn effective_effects(
        node_id: &str,
        tree: &ContentTree,
        contents: &BTreeMap<String, Content>,
    ) -> Vec<crate::effect::op::Op> {
        let Some(content) = contents.get(node_id) else {
            return Vec::new();
        };

        if tree.is_leaf(node_id) {
            // Leaf: apply all effects
            return content.effects.clone();
        }

        // Branch: apply only unclaimed effects
        // An effect is "claimed" if it appears in any child's effects list
        let children_effects: Vec<&crate::effect::op::Op> = tree
            .children_of(node_id)
            .iter()
            .filter_map(|child_id| contents.get(child_id.as_str()))
            .flat_map(|c| c.effects.iter())
            .collect();

        content
            .effects
            .iter()
            .filter(|op| !children_effects.contains(op))
            .cloned()
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
        use crate::input::entity::Character;
        let snap = Snapshot::from_parts(
            "ch01",
            vec![Entity::Character(Character {
                id: "kian".into(),
                name: None,
                traits: vec![],
                beliefs: vec![],
                desires: vec![],
                intentions: vec![],
                intent_targets: vec![],
                desire_tags: vec![],
                location: Some("wasteland".into()),
                relationships: vec![],
                inventory: vec![],
                goals: vec![],
                tags: vec![],
                description: None,
            })],
            vec![],
            vec![],
        );
        assert_eq!(snap.position, "ch01");
        assert!(snap.entity("kian").is_some());
        assert!(snap.entity("nova").is_none());
        assert_eq!(snap.characters().len(), 1);
    }
}
