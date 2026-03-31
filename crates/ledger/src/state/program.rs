//! Program assembly + execution — the top-level reasoning pipeline.
//!
//! `Program` combines `FactSet` + `RuleSet` into a complete Datalog program,
//! serializes it, and executes it via Nemo.

use std::path::Path;

use crate::LedgerError;
use crate::state::fact::{FactSet, collect_facts};
use crate::state::reasoning::{ReasoningResult, reason};
use crate::state::rule::RuleSet;
use crate::state::snapshot::Snapshot;

// ══════════════════════════════════════════════════════════════════
// Program
// ══════════════════════════════════════════════════════════════════

/// A complete Datalog program: facts + rules + exports.
#[derive(Debug, Clone)]
pub struct Program {
    pub facts: FactSet,
    pub rules: RuleSet,
}

impl Program {
    /// Build a program from a snapshot, using builtin rules
    /// and optionally loading user rules from `cards_dir/rules/*.dl`.
    pub fn from_snapshot(snapshot: &Snapshot, cards_dir: Option<&Path>) -> Self {
        let facts = collect_facts(snapshot);
        let mut rules = RuleSet::builtins();
        if let Some(dir) = cards_dir {
            rules.extend(RuleSet::load_user(dir));
        }
        Self { facts, rules }
    }

    /// Serialize the full program to Datalog text.
    pub fn to_datalog(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.facts.to_datalog());
        out.push('\n');
        out.push_str(&self.rules.to_datalog());
        out.push('\n');
        out.push_str(&self.rules.export_declarations());
        out
    }

    /// Execute the program and return reasoning results.
    pub async fn run(&self) -> Result<ReasoningResult, LedgerError> {
        reason(&self.to_datalog()).await
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::{Character, Entity, Location, Relationship};

    fn test_snapshot() -> Snapshot {
        Snapshot::from_parts(
            "ch01",
            vec![
                Entity::Character(Character {
                    id: "kian".into(),
                    name: Some("基安".into()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec!["找水".into()],
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
                Entity::Character(Character {
                    id: "nova".into(),
                    name: Some("诺娃".into()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec!["找水".into()],
                    intentions: vec![],
                    location: Some("wasteland".into()),
                    relationships: vec![],
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
        )
    }

    #[test]
    fn program_to_datalog_contains_all_sections() {
        let snap = test_snapshot();
        let program = Program::from_snapshot(&snap, None);
        let text = program.to_datalog();

        // Facts
        assert!(text.contains("character(kian)."));
        assert!(text.contains("at(kian, wasteland)."));
        assert!(text.contains("trust(kian, nova, 2)."));

        // Rules
        assert!(text.contains("can_meet(?A, ?B)"));
        assert!(text.contains("betrayal_opportunity"));

        // Exports
        assert!(text.contains("@export can_meet :- csv{}."));
    }

    #[tokio::test]
    async fn program_run_derives_can_meet() {
        let snap = test_snapshot();
        let program = Program::from_snapshot(&snap, None);
        let result = program.run().await.unwrap();

        assert!(result.has("can_meet"));
        assert!(result.contains("can_meet", &["kian", "nova"]));
    }

    #[tokio::test]
    async fn program_run_derives_would_confide() {
        let snap = test_snapshot();
        let program = Program::from_snapshot(&snap, None);
        let result = program.run().await.unwrap();

        // kian trusts nova at 2, they can meet → would_confide
        assert!(result.contains("would_confide", &["kian", "nova"]));
    }

    #[tokio::test]
    async fn program_run_derives_alliance() {
        let snap = test_snapshot();
        let program = Program::from_snapshot(&snap, None);
        let result = program.run().await.unwrap();

        // both desire "找水", affinity >= 0 → alliance_opportunity
        assert!(result.has("alliance_opportunity"));
    }

    // ── Negative emotion scenarios ──────────────────────────────

    fn hostile_snapshot() -> Snapshot {
        use crate::input::secret::Secret;
        Snapshot::from_parts(
            "ch01",
            vec![
                Entity::Character(Character {
                    id: "hero".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("arena".into()),
                    relationships: vec![Relationship {
                        target: "villain".into(),
                        role: "宿敌".into(),
                        trust: -3,
                        affinity: -3,
                        respect: -2,
                    }],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "villain".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("arena".into()),
                    relationships: vec![Relationship {
                        target: "hero".into(),
                        role: "猎物".into(),
                        trust: -2,
                        affinity: -3,
                        respect: -1,
                    }],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "subordinate".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("arena".into()),
                    relationships: vec![Relationship {
                        target: "villain".into(),
                        role: "主从".into(),
                        trust: 1,
                        affinity: -1,
                        respect: -2,
                    }],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Location(Location {
                    id: "arena".into(),
                    name: None,
                    properties: vec![],
                    connections: vec![],
                    tags: vec![],
                    description: None,
                }),
            ],
            vec![Secret {
                id: "hero_weakness".into(),
                content: "hero has a weakness".into(),
                known_by: vec!["villain".into()],
                revealed_to_reader: true,
                dramatic_function: None,
            }],
            vec![],
        )
    }

    #[tokio::test]
    async fn personal_enemy_from_low_affinity() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // affinity <= -2 → personal_enemy
        assert!(result.contains("personal_enemy", &["hero", "villain"]));
        assert!(result.contains("personal_enemy", &["villain", "hero"]));
    }

    #[tokio::test]
    async fn danger_from_personal_enemy_meeting() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // personal_enemy + can_meet → danger
        assert!(result.contains("danger", &["hero", "villain"]));
    }

    #[tokio::test]
    async fn betrayal_opportunity_from_low_trust() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // villain knows hero_weakness, trust <= -1, can_meet → betrayal_opportunity
        let triples = result.triples("betrayal_opportunity");
        assert!(triples.iter().any(|(p, v, _)| *p == "villain" && *v == "hero"));
    }

    #[tokio::test]
    async fn rebellion_seed_from_low_respect_and_affinity() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // subordinate → villain: respect <= -1, affinity <= -1 → rebellion_seed
        assert!(result.contains("rebellion_seed", &["subordinate", "villain"]));
    }

    #[tokio::test]
    async fn dramatic_irony_from_secret() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // hero_weakness: villain knows, reader knows, hero doesn't → dramatic_irony
        assert!(result.contains("dramatic_irony", &["hero_weakness", "hero"]));
    }

    #[tokio::test]
    async fn would_obey_not_derived_for_low_respect() {
        let snap = hostile_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // subordinate respect for villain is -2, should NOT would_obey
        assert!(!result.contains("would_obey", &["subordinate", "villain"]));
    }

    // ── High respect / sacrifice scenarios ───────────────────────

    fn devotion_snapshot() -> Snapshot {
        Snapshot::from_parts(
            "ch01",
            vec![
                Entity::Character(Character {
                    id: "disciple".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("temple".into()),
                    relationships: vec![Relationship {
                        target: "master".into(),
                        role: "师父".into(),
                        trust: 3,
                        affinity: 3,
                        respect: 3,
                    }],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "master".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("temple".into()),
                    relationships: vec![],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Location(Location {
                    id: "temple".into(),
                    name: None,
                    properties: vec![],
                    connections: vec![],
                    tags: vec![],
                    description: None,
                }),
            ],
            vec![],
            vec![],
        )
    }

    #[tokio::test]
    async fn would_obey_from_high_respect() {
        let snap = devotion_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        assert!(result.contains("would_obey", &["disciple", "master"]));
    }

    #[tokio::test]
    async fn would_sacrifice_from_high_affinity_and_trust() {
        let snap = devotion_snapshot();
        let result = Program::from_snapshot(&snap, None).run().await.unwrap();
        // affinity >= 3, trust >= 1 → would_sacrifice
        assert!(result.contains("would_sacrifice", &["disciple", "master"]));
    }
}
