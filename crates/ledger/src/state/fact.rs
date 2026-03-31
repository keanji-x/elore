//! Typed Datalog fact generation — single source of truth for all facts.
//!
//! `FactSet` collects facts with typed arguments, deduplicates, and serializes
//! to Datalog text. `collect_facts(snapshot)` is the sole entry point for fact
//! generation, replacing the scattered `to_datalog()` methods.

use indexmap::IndexSet;

use crate::input::entity::Entity;
use crate::input::goal::GoalEntity;
use crate::input::secret::Secret;
use crate::state::snapshot::Snapshot;

// ══════════════════════════════════════════════════════════════════
// Arg — typed fact argument
// ══════════════════════════════════════════════════════════════════

/// A single argument in a Datalog fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Arg {
    /// Bare identifier: `kian`, `wasteland`. Must be ASCII alphanumeric + underscore.
    Id(String),
    /// Quoted string: `"师徒"`, `"坚韧"`. Handles escaping.
    Str(String),
    /// Integer literal: `3`, `-2`.
    Int(i64),
}

impl Arg {
    /// Smart constructor: if the value is a valid bare identifier, use `Id`;
    /// otherwise use `Str`.
    pub fn auto(s: &str) -> Self {
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            Self::Id(s.to_string())
        } else {
            Self::Str(s.to_string())
        }
    }

    fn to_datalog(&self) -> String {
        match self {
            Arg::Id(s) => s.clone(),
            Arg::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            Arg::Int(n) => n.to_string(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// FactSet — ordered, deduplicated fact collection
// ══════════════════════════════════════════════════════════════════

/// A Datalog fact: predicate name + typed arguments.
type Fact = (String, Vec<Arg>);

/// Ordered, deduplicated collection of Datalog facts.
#[derive(Debug, Clone, Default)]
pub struct FactSet {
    facts: IndexSet<Fact>,
}

impl FactSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a fact. Duplicates are silently ignored.
    pub fn add(&mut self, predicate: &str, args: Vec<Arg>) {
        self.facts.insert((predicate.to_string(), args));
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Serialize all facts to a Datalog program fragment.
    pub fn to_datalog(&self) -> String {
        let mut out = String::with_capacity(self.facts.len() * 40);
        out.push_str("% === Facts ===\n");
        for (pred, args) in &self.facts {
            out.push_str(pred);
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&arg.to_datalog());
            }
            out.push_str(").\n");
        }
        out
    }
}

// ══════════════════════════════════════════════════════════════════
// collect_facts — single entry point
// ══════════════════════════════════════════════════════════════════

/// Generate all Datalog facts from a snapshot. Single entry point.
pub fn collect_facts(snapshot: &Snapshot) -> FactSet {
    let mut facts = FactSet::new();
    for entity in &snapshot.entities {
        emit_entity_facts(&mut facts, entity);
    }
    emit_goal_facts(&mut facts, &snapshot.goal_entities);
    emit_secret_facts(&mut facts, &snapshot.secrets);
    facts
}

// ── Entity facts ────────────────────────────────────────────────

fn emit_entity_facts(f: &mut FactSet, entity: &Entity) {
    let id = entity.id();

    f.add("entity", vec![Arg::Id(id.into()), Arg::Id(entity.entity_type().into())]);

    match entity {
        Entity::Character(c) => {
            f.add("character", vec![Arg::Id(id.into())]);
            for t in &c.traits {
                f.add("trait", vec![Arg::Id(id.into()), Arg::auto(t)]);
            }
            for b in &c.beliefs {
                f.add("believes", vec![Arg::Id(id.into()), Arg::auto(b)]);
            }
            for d in &c.desires {
                f.add("desires", vec![Arg::Id(id.into()), Arg::auto(d)]);
            }
            for i in &c.intentions {
                f.add("intends", vec![Arg::Id(id.into()), Arg::auto(i)]);
            }
            if let Some(loc) = &c.location {
                f.add("at", vec![Arg::Id(id.into()), Arg::Id(loc.clone())]);
            }
            for r in &c.relationships {
                f.add("role", vec![Arg::Id(id.into()), Arg::Id(r.target.clone()), Arg::auto(&r.role)]);
                f.add("trust", vec![Arg::Id(id.into()), Arg::Id(r.target.clone()), Arg::Int(r.trust as i64)]);
                f.add("affinity", vec![Arg::Id(id.into()), Arg::Id(r.target.clone()), Arg::Int(r.affinity as i64)]);
                f.add("respect", vec![Arg::Id(id.into()), Arg::Id(r.target.clone()), Arg::Int(r.respect as i64)]);
            }
            for item in &c.inventory {
                f.add("has", vec![Arg::Id(id.into()), Arg::auto(item)]);
            }
            if let Some(name) = &c.name {
                f.add("name", vec![Arg::Id(id.into()), Arg::auto(name)]);
            }
        }
        Entity::Location(l) => {
            f.add("location", vec![Arg::Id(id.into())]);
            for p in &l.properties {
                f.add("property", vec![Arg::Id(id.into()), Arg::auto(p)]);
            }
            for conn in &l.connections {
                f.add("connected", vec![Arg::Id(id.into()), Arg::Id(conn.clone())]);
                f.add("connected", vec![Arg::Id(conn.clone()), Arg::Id(id.into())]);
            }
            if let Some(name) = &l.name {
                f.add("name", vec![Arg::Id(id.into()), Arg::auto(name)]);
            }
        }
        Entity::Faction(fa) => {
            f.add("faction", vec![Arg::Id(id.into())]);
            if let Some(align) = &fa.alignment {
                f.add("alignment", vec![Arg::Id(id.into()), Arg::auto(align)]);
            }
            for r in &fa.rivals {
                f.add("rival", vec![Arg::Id(id.into()), Arg::Id(r.clone())]);
            }
            for m in &fa.members {
                f.add("member", vec![Arg::Id(m.clone()), Arg::Id(id.into())]);
            }
            if let Some(name) = &fa.name {
                f.add("name", vec![Arg::Id(id.into()), Arg::auto(name)]);
            }
        }
    }
}

// ── Goal facts ──────────────────────────────────────────────────

fn emit_goal_facts(f: &mut FactSet, entities: &[GoalEntity]) {
    for entity in entities {
        for goal in &entity.goals {
            emit_goal_tree(f, &entity.id, goal, None);
        }
    }
}

fn emit_goal_tree(
    f: &mut FactSet,
    owner: &str,
    goal: &crate::input::goal::Goal,
    parent: Option<&str>,
) {
    let gid = Arg::auto(&goal.id);

    f.add("want", vec![Arg::Id(owner.into()), gid.clone(), Arg::Str(goal.want.clone())]);
    f.add("goal_status", vec![Arg::Id(owner.into()), gid.clone(), Arg::Id(goal.status.label().into())]);

    if goal.solution.is_some() {
        f.add("has_solution", vec![Arg::Id(owner.into()), gid.clone()]);
    }
    if let Some(problem) = &goal.problem {
        f.add("goal_problem", vec![Arg::Id(owner.into()), gid.clone(), Arg::Str(problem.clone())]);
    }
    if let Some(parent_id) = parent {
        f.add("child_of", vec![gid.clone(), Arg::auto(parent_id)]);
    }

    for ref_path in &goal.blocked_by {
        if let Some((ref_owner, ref_goal)) = ref_path.split_once('/') {
            f.add("blocks", vec![
                Arg::Id(ref_owner.into()), Arg::auto(ref_goal),
                Arg::Id(owner.into()), gid.clone(),
            ]);
        }
    }
    for ref_path in &goal.conflicts_with {
        if let Some((ref_owner, ref_goal)) = ref_path.split_once('/') {
            f.add("conflicts", vec![
                Arg::Id(owner.into()), gid.clone(),
                Arg::Id(ref_owner.into()), Arg::auto(ref_goal),
            ]);
        }
    }

    for child in &goal.children {
        emit_goal_tree(f, owner, child, Some(&goal.id));
    }
}

// ── Secret facts ────────────────────────────────────────────────

fn emit_secret_facts(f: &mut FactSet, secrets: &[Secret]) {
    for secret in secrets {
        let sid = Arg::Id(secret.id.clone());
        f.add("secret", vec![sid.clone()]);

        for char_id in &secret.known_by {
            f.add("secret_known_by", vec![sid.clone(), Arg::Id(char_id.clone())]);
        }

        if secret.revealed_to_reader {
            f.add("secret_revealed_to_reader", vec![sid.clone()]);
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::{Character, Location, Relationship};

    #[test]
    fn arg_auto_id() {
        assert_eq!(Arg::auto("kian"), Arg::Id("kian".into()));
        assert_eq!(Arg::auto("sector_4"), Arg::Id("sector_4".into()));
    }

    #[test]
    fn arg_auto_str() {
        assert_eq!(Arg::auto("师徒"), Arg::Str("师徒".into()));
        assert_eq!(Arg::auto("hello world"), Arg::Str("hello world".into()));
    }

    #[test]
    fn factset_dedup() {
        let mut fs = FactSet::new();
        fs.add("at", vec![Arg::Id("kian".into()), Arg::Id("wasteland".into())]);
        fs.add("at", vec![Arg::Id("kian".into()), Arg::Id("wasteland".into())]);
        assert_eq!(fs.len(), 1);
    }

    #[test]
    fn factset_to_datalog_format() {
        let mut fs = FactSet::new();
        fs.add("trust", vec![Arg::Id("kian".into()), Arg::Id("nova".into()), Arg::Int(3)]);
        fs.add("role", vec![Arg::Id("kian".into()), Arg::Id("nova".into()), Arg::Str("师徒".into())]);
        let text = fs.to_datalog();
        assert!(text.contains("trust(kian, nova, 3)."));
        assert!(text.contains("role(kian, nova, \"师徒\")."));
    }

    #[test]
    fn collect_facts_from_entities() {
        let snapshot = Snapshot::from_parts(
            "ch01",
            vec![
                Entity::Character(Character {
                    id: "kian".into(),
                    name: Some("基安".into()),
                    traits: vec!["勇敢".into()],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("wasteland".into()),
                    relationships: vec![Relationship {
                        target: "nova".into(),
                        role: "ally".into(),
                        trust: 2,
                        affinity: 1,
                        respect: 0,
                    }],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Location(Location {
                    id: "wasteland".into(),
                    name: None,
                    properties: vec![],
                    connections: vec!["oasis".into()],
                    tags: vec![],
                    description: None,
                }),
            ],
            vec![],
            vec![],
        );

        let facts = collect_facts(&snapshot);
        let text = facts.to_datalog();
        assert!(text.contains("character(kian)."));
        assert!(text.contains("at(kian, wasteland)."));
        assert!(text.contains("trust(kian, nova, 2)."));
        assert!(text.contains("affinity(kian, nova, 1)."));
        assert!(text.contains("connected(wasteland, oasis)."));
        assert!(text.contains("connected(oasis, wasteland)."));
        // No duplicates from graph.to_datalog() — single source
    }
}
