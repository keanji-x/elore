//! Entity schemas and JSON→Datalog translation.
//!
//! Entities are JSON files in `.everlore/entities/`.
//! Each entity has a type (character, location, faction) and a unique id.
//! `to_datalog()` translates an entity into Datalog fact strings for reasoning.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A relationship edge to another entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub target: String,
    pub rel: String,
}

/// Unified entity structure — all fields optional except `type` + `id`.
///
/// This is the **only human-editable surface** of the entire system.
/// Character JSON defines initial state, goals, beliefs, and environment view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,

    #[serde(default)]
    pub name: Option<String>,

    // ── Character fields ─────────────────────────────────────────
    #[serde(default)]
    pub traits: Vec<String>,
    #[serde(default)]
    pub beliefs: Vec<String>,
    #[serde(default)]
    pub desires: Vec<String>,
    #[serde(default)]
    pub intentions: Vec<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub inventory: Vec<String>,

    // ── Faction fields ───────────────────────────────────────────
    #[serde(default)]
    pub alignment: Option<String>,
    #[serde(default)]
    pub rivals: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,

    // ── Location fields ──────────────────────────────────────────
    #[serde(default)]
    pub properties: Vec<String>,
    #[serde(default)]
    pub connections: Vec<String>,

    // ── Tag filtering ────────────────────────────────────────────
    #[serde(default)]
    pub tags: Vec<String>,
}

// ══════════════════════════════════════════════════════════════════
// Datalog translation
// ══════════════════════════════════════════════════════════════════

impl Entity {
    /// Translate this entity into Datalog fact lines.
    pub fn to_datalog(&self) -> Vec<String> {
        let id = &self.id;
        let mut facts = Vec::new();

        facts.push(format!("entity({id}, {}).", self.entity_type));

        match self.entity_type.as_str() {
            "character" => self.character_facts(id, &mut facts),
            "location" => self.location_facts(id, &mut facts),
            "faction" => self.faction_facts(id, &mut facts),
            _ => {}
        }

        facts
    }

    fn character_facts(&self, id: &str, facts: &mut Vec<String>) {
        facts.push(format!("character({id})."));

        for t in &self.traits {
            facts.push(format!("trait({id}, {}).", quote(t)));
        }
        for b in &self.beliefs {
            facts.push(format!("believes({id}, {}).", quote(b)));
        }
        for d in &self.desires {
            facts.push(format!("desires({id}, {}).", quote(d)));
        }
        for i in &self.intentions {
            facts.push(format!("intends({id}, {}).", quote(i)));
        }
        if let Some(loc) = &self.location {
            facts.push(format!("at({id}, {loc})."));
        }
        for r in &self.relationships {
            facts.push(format!("rel({id}, {}, {}).", r.target, r.rel));
        }
        for item in &self.inventory {
            facts.push(format!("has({id}, {}).", quote(item)));
        }
        if let Some(name) = &self.name {
            facts.push(format!("name({id}, {}).", quote(name)));
        }
    }

    fn location_facts(&self, id: &str, facts: &mut Vec<String>) {
        facts.push(format!("location({id})."));

        for p in &self.properties {
            facts.push(format!("property({id}, {}).", quote(p)));
        }
        for c in &self.connections {
            facts.push(format!("connected({id}, {c})."));
            facts.push(format!("connected({c}, {id})."));
        }
        if let Some(name) = &self.name {
            facts.push(format!("name({id}, {}).", quote(name)));
        }
    }

    fn faction_facts(&self, id: &str, facts: &mut Vec<String>) {
        facts.push(format!("faction({id})."));

        if let Some(align) = &self.alignment {
            facts.push(format!("alignment({id}, {}).", quote(align)));
        }
        for r in &self.rivals {
            facts.push(format!("rival({id}, {r})."));
        }
        for m in &self.members {
            facts.push(format!("member({m}, {id})."));
        }
        if let Some(name) = &self.name {
            facts.push(format!("name({id}, {}).", quote(name)));
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Loading & filtering
// ══════════════════════════════════════════════════════════════════

/// Load all entity JSON files from a directory.
pub fn load_entities(dir: &Path) -> Result<Vec<Entity>, LedgerError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entities = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let content = std::fs::read_to_string(&path)?;
            let entity: Entity = serde_json::from_str(&content)?;
            entities.push(entity);
        }
    }
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entities)
}

/// Filter entities by active tags.
/// Entities with no tags are always included.
/// Entities with tags need at least one tag in the active set.
pub fn filter_by_tags(entities: Vec<Entity>, active_tags: &BTreeSet<String>) -> Vec<Entity> {
    if active_tags.is_empty() {
        return entities;
    }
    entities
        .into_iter()
        .filter(|e| e.tags.is_empty() || e.tags.iter().any(|t| active_tags.contains(t)))
        .collect()
}

/// Translate a list of entities into a combined Datalog facts string.
pub fn translate_to_datalog(entities: &[Entity]) -> String {
    let mut output = String::from("% Auto-generated entity facts\n\n");
    for entity in entities {
        output.push_str(&format!(
            "% --- {} ({}) ---\n",
            entity.id, entity.entity_type
        ));
        for fact in entity.to_datalog() {
            output.push_str(&fact);
            output.push('\n');
        }
        output.push('\n');
    }
    output
}

/// Built-in reasoning rules that every project gets.
pub fn builtin_rules() -> &'static str {
    r#"% === Built-in Rules ===

% Social: who can meet (same location)
can_meet(?A, ?B) :- at(?A, ?P), at(?B, ?P), ?A != ?B, character(?A), character(?B).

% Social: enemy detection (rival factions)
enemy(?A, ?B) :- member(?A, ?S1), member(?B, ?S2), rival(?S1, ?S2), ?A != ?B.

% Danger: enemies meeting
danger(?A, ?B) :- can_meet(?A, ?B), enemy(?A, ?B).

% Location: reachable (transitive connections)
reachable(?A, ?B) :- connected(?A, ?B).
reachable(?A, ?C) :- connected(?A, ?B), reachable(?B, ?C).

% Social: heard-of via relationships
heard_of(?A, ?C) :- rel(?A, ?B, knows), rel(?B, ?C, knows), ?A != ?C.
"#
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

/// Quote a string value for Datalog. If it's a simple identifier, leave it bare.
fn quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('"', "\\\""))
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_character() -> Entity {
        Entity {
            entity_type: "character".into(),
            id: "kian".into(),
            name: Some("基安".into()),
            traits: vec!["废土拾荒者".into(), "极其渴望水源".into()],
            beliefs: vec!["绿洲的财阀藏匿了世界上最后一条干净的地下水脉".into()],
            desires: vec![],
            intentions: vec![],
            location: Some("wasteland".into()),
            relationships: vec![],
            inventory: vec!["旧式防毒面具".into(), "电磁短刀".into()],
            alignment: None,
            rivals: vec![],
            members: vec![],
            properties: vec![],
            connections: vec![],
            tags: vec!["ch01".into(), "ch02".into()],
        }
    }

    #[test]
    fn entity_to_datalog_character() {
        let e = make_character();
        let facts = e.to_datalog();
        assert!(facts.contains(&"character(kian).".to_string()));
        assert!(facts.contains(&"at(kian, wasteland).".to_string()));
        assert!(facts.iter().any(|f| f.starts_with("trait(kian,")));
        assert!(facts.iter().any(|f| f.starts_with("has(kian,")));
    }

    #[test]
    fn filter_by_tags_empty_includes_all() {
        let entities = vec![make_character()];
        let tags = BTreeSet::new();
        let filtered = filter_by_tags(entities.clone(), &tags);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_by_tags_matching() {
        let entities = vec![make_character()];
        let tags = BTreeSet::from(["ch01".to_string()]);
        let filtered = filter_by_tags(entities, &tags);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_by_tags_no_match() {
        let entities = vec![make_character()];
        let tags = BTreeSet::from(["ch99".to_string()]);
        let filtered = filter_by_tags(entities, &tags);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn quote_simple_id() {
        assert_eq!(quote("kian"), "kian");
        assert_eq!(quote("wasteland"), "wasteland");
    }

    #[test]
    fn quote_unicode() {
        assert_eq!(quote("废土拾荒者"), "\"废土拾荒者\"");
    }
}
