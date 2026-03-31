//! Entity schemas and JSON→Datalog translation.
//!
//! Entities are JSON files in `.everlore/entities/`.
//! Each entity has a type (character, location, faction) and a unique id.
//! `to_datalog()` translates an entity into Datalog fact strings for reasoning.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;
use crate::input::goal::Goal;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A relationship edge to another entity.
///
/// Models interpersonal dynamics via three emotion axes (IPC + trust):
/// - **trust** (信任): secret sharing, betrayal potential (-3 ~ +3)
/// - **affinity** (亲疏): alliance, sacrifice, hostility (-3 ~ +3)
/// - **respect** (敬畏): power dynamics, obedience, rebellion (-3 ~ +3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub target: String,
    /// Social role label (e.g. "师徒", "死敌").
    pub role: String,
    /// Trust axis: -3 (deep distrust) to +3 (unconditional trust). Default 0.
    #[serde(default)]
    pub trust: i8,
    /// Affinity axis: -3 (hatred) to +3 (devotion). Default 0.
    #[serde(default)]
    pub affinity: i8,
    /// Respect axis: -3 (contempt) to +3 (reverence). Default 0.
    #[serde(default)]
    pub respect: i8,
}

/// Entity enum — each variant contains only its relevant fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Entity {
    Character(Character),
    Location(Location),
    Faction(Faction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
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
    #[serde(default)]
    pub goals: Vec<Goal>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub properties: Vec<String>,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Faction {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub alignment: Option<String>,
    #[serde(default)]
    pub rivals: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ══════════════════════════════════════════════════════════════════
// Common accessors
// ══════════════════════════════════════════════════════════════════

impl Entity {
    pub fn id(&self) -> &str {
        match self {
            Entity::Character(c) => &c.id,
            Entity::Location(l) => &l.id,
            Entity::Faction(f) => &f.id,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Entity::Character(c) => c.name.as_deref(),
            Entity::Location(l) => l.name.as_deref(),
            Entity::Faction(f) => f.name.as_deref(),
        }
    }

    pub fn entity_type(&self) -> &str {
        match self {
            Entity::Character(_) => "character",
            Entity::Location(_) => "location",
            Entity::Faction(_) => "faction",
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            Entity::Character(c) => &c.tags,
            Entity::Location(l) => &l.tags,
            Entity::Faction(f) => &f.tags,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            Entity::Character(c) => c.description.as_deref(),
            Entity::Location(l) => l.description.as_deref(),
            Entity::Faction(f) => f.description.as_deref(),
        }
    }

    pub fn set_description(&mut self, desc: Option<String>) {
        match self {
            Entity::Character(c) => c.description = desc,
            Entity::Location(l) => l.description = desc,
            Entity::Faction(f) => f.description = desc,
        }
    }

    pub fn is_character(&self) -> bool {
        matches!(self, Entity::Character(_))
    }

    pub fn is_location(&self) -> bool {
        matches!(self, Entity::Location(_))
    }

    pub fn is_faction(&self) -> bool {
        matches!(self, Entity::Faction(_))
    }

    pub fn as_character(&self) -> Option<&Character> {
        match self {
            Entity::Character(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_character_mut(&mut self) -> Option<&mut Character> {
        match self {
            Entity::Character(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_location(&self) -> Option<&Location> {
        match self {
            Entity::Location(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_faction(&self) -> Option<&Faction> {
        match self {
            Entity::Faction(f) => Some(f),
            _ => None,
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Datalog translation
// ══════════════════════════════════════════════════════════════════

impl Entity {
    /// Translate this entity into Datalog fact lines.
    pub fn to_datalog(&self) -> Vec<String> {
        let id = self.id();
        let mut facts = Vec::new();

        facts.push(format!("entity({id}, {}).", self.entity_type()));

        match self {
            Entity::Character(c) => {
                facts.push(format!("character({id})."));
                for t in &c.traits {
                    facts.push(format!("trait({id}, {}).", quote(t)));
                }
                for b in &c.beliefs {
                    facts.push(format!("believes({id}, {}).", quote(b)));
                }
                for d in &c.desires {
                    facts.push(format!("desires({id}, {}).", quote(d)));
                }
                for i in &c.intentions {
                    facts.push(format!("intends({id}, {}).", quote(i)));
                }
                if let Some(loc) = &c.location {
                    facts.push(format!("at({id}, {loc})."));
                }
                for r in &c.relationships {
                    facts.push(format!("role({id}, {}, {}).", r.target, quote(&r.role)));
                    facts.push(format!("trust({id}, {}, {}).", r.target, r.trust));
                    facts.push(format!("affinity({id}, {}, {}).", r.target, r.affinity));
                    facts.push(format!("respect({id}, {}, {}).", r.target, r.respect));
                }
                for item in &c.inventory {
                    facts.push(format!("has({id}, {}).", quote(item)));
                }
                if let Some(name) = &c.name {
                    facts.push(format!("name({id}, {}).", quote(name)));
                }
            }
            Entity::Location(l) => {
                facts.push(format!("location({id})."));
                for p in &l.properties {
                    facts.push(format!("property({id}, {}).", quote(p)));
                }
                for c in &l.connections {
                    facts.push(format!("connected({id}, {c})."));
                    facts.push(format!("connected({c}, {id})."));
                }
                if let Some(name) = &l.name {
                    facts.push(format!("name({id}, {}).", quote(name)));
                }
            }
            Entity::Faction(f) => {
                facts.push(format!("faction({id})."));
                if let Some(align) = &f.alignment {
                    facts.push(format!("alignment({id}, {}).", quote(align)));
                }
                for r in &f.rivals {
                    facts.push(format!("rival({id}, {r})."));
                }
                for m in &f.members {
                    facts.push(format!("member({m}, {id})."));
                }
                if let Some(name) = &f.name {
                    facts.push(format!("name({id}, {}).", quote(name)));
                }
            }
        }

        facts
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
    entities.sort_by(|a, b| a.id().cmp(b.id()));
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
        .filter(|e| e.tags().is_empty() || e.tags().iter().any(|t| active_tags.contains(t)))
        .collect()
}

/// Translate a list of entities into a combined Datalog facts string.
pub fn translate_to_datalog(entities: &[Entity]) -> String {
    let mut output = String::from("% Auto-generated entity facts\n\n");
    for entity in entities {
        output.push_str(&format!(
            "% --- {} ({}) ---\n",
            entity.id(),
            entity.entity_type()
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

% Social: personal enemy (affinity <= -2)
personal_enemy(?A, ?B) :- affinity(?A, ?B, ?V), ?V <= -2, character(?A), character(?B).

% Danger: enemies meeting (faction or personal)
danger(?A, ?B) :- can_meet(?A, ?B), enemy(?A, ?B).
danger(?A, ?B) :- can_meet(?A, ?B), personal_enemy(?A, ?B).

% Location: reachable (transitive connections)
reachable(?A, ?B) :- connected(?A, ?B).
reachable(?A, ?C) :- connected(?A, ?B), reachable(?B, ?C).

% Social: heard-of via relationships
heard_of(?A, ?C) :- role(?A, ?B, ?R1), role(?B, ?C, ?R2), ?A != ?C.

% Trust: would confide a secret (trust >= 2 + can meet)
would_confide(?A, ?B) :- trust(?A, ?B, ?T), ?T >= 2, can_meet(?A, ?B).

% Trust: would obey (respect >= 2)
would_obey(?A, ?B) :- respect(?A, ?B, ?R), ?R >= 2.

% Trust: would sacrifice (affinity >= 3 + trust >= 1)
would_sacrifice(?A, ?B) :- affinity(?A, ?B, ?Af), ?Af >= 3, trust(?A, ?B, ?T), ?T >= 1.

% Conflict: rebellion seed (low respect + low affinity)
rebellion_seed(?Sub, ?Sup) :- respect(?Sub, ?Sup, ?R), ?R <= -1, affinity(?Sub, ?Sup, ?A), ?A <= -1.
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
        Entity::Character(Character {
            id: "kian".into(),
            name: Some("基安".into()),
            traits: vec!["废土拾荒者".into(), "极其渴望水源".into()],
            beliefs: vec!["绿洲的财阀藏匿了世界上最后一条干净的地下水脉".into()],
            desires: vec![],
            intentions: vec![],
            location: Some("wasteland".into()),
            relationships: vec![],
            inventory: vec!["旧式防毒面具".into(), "电磁短刀".into()],
            goals: vec![],
            tags: vec!["ch01".into(), "ch02".into()],
            description: None,
        })
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
