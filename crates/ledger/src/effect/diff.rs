//! Snapshot diff — comparing world states for change propagation.
//!
//! Used for the "non-linear editing" (吃书) mechanism: when effects
//! change at chapter N, diff propagation determines which subsequent
//! chapters need re-generation.

use std::collections::BTreeSet;

use crate::input::entity::Entity;
use crate::state::snapshot::Snapshot;

// ══════════════════════════════════════════════════════════════════
// Diff types
// ══════════════════════════════════════════════════════════════════

/// Diff between two snapshots.
#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub chapter: String,
    pub added_entities: Vec<String>,
    pub removed_entities: Vec<String>,
    pub entity_diffs: Vec<EntityDiff>,
}

/// Diff for a single entity between two snapshots.
#[derive(Debug, Clone)]
pub struct EntityDiff {
    pub id: String,
    pub entity_type: String,
    pub location_change: Option<(Option<String>, Option<String>)>,
    pub added_traits: Vec<String>,
    pub removed_traits: Vec<String>,
    pub added_beliefs: Vec<String>,
    pub removed_beliefs: Vec<String>,
    pub added_relationships: Vec<(String, String)>,
    pub removed_relationships: Vec<(String, String)>,
    pub added_inventory: Vec<String>,
    pub removed_inventory: Vec<String>,
}

// ══════════════════════════════════════════════════════════════════
// Computation
// ══════════════════════════════════════════════════════════════════

impl SnapshotDiff {
    /// Compute the diff between two snapshots.
    pub fn compute(old: &Snapshot, new: &Snapshot) -> Self {
        let old_ids: BTreeSet<&str> = old.entities.iter().map(|e| e.id.as_str()).collect();
        let new_ids: BTreeSet<&str> = new.entities.iter().map(|e| e.id.as_str()).collect();

        let added: Vec<String> = new_ids
            .difference(&old_ids)
            .map(|s| s.to_string())
            .collect();
        let removed: Vec<String> = old_ids
            .difference(&new_ids)
            .map(|s| s.to_string())
            .collect();

        // Compare shared entities
        let mut entity_diffs = Vec::new();
        for id in old_ids.intersection(&new_ids) {
            let old_e = old.entities.iter().find(|e| e.id == *id).unwrap();
            let new_e = new.entities.iter().find(|e| e.id == *id).unwrap();
            let diff = diff_entity(old_e, new_e);
            if !diff.is_empty() {
                entity_diffs.push(diff);
            }
        }

        Self {
            chapter: new.chapter.clone(),
            added_entities: added,
            removed_entities: removed,
            entity_diffs,
        }
    }

    /// True if no differences were found.
    pub fn is_empty(&self) -> bool {
        self.added_entities.is_empty()
            && self.removed_entities.is_empty()
            && self.entity_diffs.is_empty()
    }

    /// Render as human-readable text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Diff for {}:\n", self.chapter));

        if self.is_empty() {
            out.push_str("  (no changes)\n");
            return out;
        }

        for id in &self.added_entities {
            out.push_str(&format!("  + NEW: {id}\n"));
        }
        for id in &self.removed_entities {
            out.push_str(&format!("  - GONE: {id}\n"));
        }
        for diff in &self.entity_diffs {
            out.push_str(&format!("  Δ {} ({}):\n", diff.id, diff.entity_type));
            if let Some((old, new)) = &diff.location_change {
                let old_s = old.as_deref().unwrap_or("?");
                let new_s = new.as_deref().unwrap_or("?");
                out.push_str(&format!("    location: {old_s} → {new_s}\n"));
            }
            for t in &diff.added_traits {
                out.push_str(&format!("    + trait: {t}\n"));
            }
            for t in &diff.removed_traits {
                out.push_str(&format!("    - trait: {t}\n"));
            }
            for b in &diff.added_beliefs {
                out.push_str(&format!("    + belief: {b}\n"));
            }
            for b in &diff.removed_beliefs {
                out.push_str(&format!("    - belief: {b}\n"));
            }
            for (t, r) in &diff.added_relationships {
                out.push_str(&format!("    + rel: {r}({t})\n"));
            }
            for (t, r) in &diff.removed_relationships {
                out.push_str(&format!("    - rel: {r}({t})\n"));
            }
            for i in &diff.added_inventory {
                out.push_str(&format!("    + item: {i}\n"));
            }
            for i in &diff.removed_inventory {
                out.push_str(&format!("    - item: {i}\n"));
            }
        }
        out
    }

    /// Get all entity IDs that have any changes (for dependency filtering).
    pub fn changed_entity_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        ids.extend(self.added_entities.iter().cloned());
        ids.extend(self.removed_entities.iter().cloned());
        for diff in &self.entity_diffs {
            ids.insert(diff.id.clone());
        }
        ids
    }
}

impl EntityDiff {
    fn is_empty(&self) -> bool {
        self.location_change.is_none()
            && self.added_traits.is_empty()
            && self.removed_traits.is_empty()
            && self.added_beliefs.is_empty()
            && self.removed_beliefs.is_empty()
            && self.added_relationships.is_empty()
            && self.removed_relationships.is_empty()
            && self.added_inventory.is_empty()
            && self.removed_inventory.is_empty()
    }
}

fn diff_entity(old: &Entity, new: &Entity) -> EntityDiff {
    let old_traits: BTreeSet<&String> = old.traits.iter().collect();
    let new_traits: BTreeSet<&String> = new.traits.iter().collect();

    let old_beliefs: BTreeSet<&String> = old.beliefs.iter().collect();
    let new_beliefs: BTreeSet<&String> = new.beliefs.iter().collect();

    let old_rels: BTreeSet<(String, String)> = old
        .relationships
        .iter()
        .map(|r| (r.target.clone(), r.rel.clone()))
        .collect();
    let new_rels: BTreeSet<(String, String)> = new
        .relationships
        .iter()
        .map(|r| (r.target.clone(), r.rel.clone()))
        .collect();

    let old_inv: BTreeSet<&String> = old.inventory.iter().collect();
    let new_inv: BTreeSet<&String> = new.inventory.iter().collect();

    let location_change = if old.location != new.location {
        Some((old.location.clone(), new.location.clone()))
    } else {
        None
    };

    EntityDiff {
        id: old.id.clone(),
        entity_type: old.entity_type.clone(),
        location_change,
        added_traits: new_traits
            .difference(&old_traits)
            .map(|s| (*s).clone())
            .collect(),
        removed_traits: old_traits
            .difference(&new_traits)
            .map(|s| (*s).clone())
            .collect(),
        added_beliefs: new_beliefs
            .difference(&old_beliefs)
            .map(|s| (*s).clone())
            .collect(),
        removed_beliefs: old_beliefs
            .difference(&new_beliefs)
            .map(|s| (*s).clone())
            .collect(),
        added_relationships: new_rels.difference(&old_rels).cloned().collect(),
        removed_relationships: old_rels.difference(&new_rels).cloned().collect(),
        added_inventory: new_inv.difference(&old_inv).map(|s| (*s).clone()).collect(),
        removed_inventory: old_inv.difference(&new_inv).map(|s| (*s).clone()).collect(),
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: &str, location: &str, traits: Vec<&str>) -> Entity {
        Entity {
            entity_type: "character".into(),
            id: id.into(),
            name: None,
            traits: traits.into_iter().map(|s| s.into()).collect(),
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: Some(location.into()),
            relationships: vec![],
            inventory: vec![],
            alignment: None,
            rivals: vec![],
            members: vec![],
            properties: vec![],
            connections: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn diff_detects_trait_change() {
        let old = make_entity("kian", "wasteland", vec!["brave"]);
        let new = make_entity("kian", "wasteland", vec!["brave", "tracked"]);
        let diff = diff_entity(&old, &new);
        assert_eq!(diff.added_traits, vec!["tracked"]);
        assert!(diff.removed_traits.is_empty());
    }

    #[test]
    fn diff_detects_location_change() {
        let old = make_entity("kian", "wasteland", vec![]);
        let new = make_entity("kian", "oasis", vec![]);
        let diff = diff_entity(&old, &new);
        assert_eq!(
            diff.location_change,
            Some((Some("wasteland".into()), Some("oasis".into())))
        );
    }

    #[test]
    fn diff_empty_when_same() {
        let e = make_entity("kian", "wasteland", vec!["brave"]);
        let diff = diff_entity(&e, &e);
        assert!(diff.is_empty());
    }

    #[test]
    fn snapshot_diff_empty() {
        let snap = Snapshot {
            chapter: "ch01".into(),
            entities: vec![make_entity("kian", "wasteland", vec![])],
            secrets: vec![],
            goal_entities: vec![],
        };
        let diff = SnapshotDiff::compute(&snap, &snap);
        assert!(diff.is_empty());
    }
}
