//! L1 constraint checking — invariants and exit_state assertions on Snapshot.

use crate::state::phase::StateAssertion;
use crate::state::snapshot::Snapshot;

/// Check a list of assertions against a snapshot.
/// Returns (passed, failed_reasons).
pub fn check_assertions(snapshot: &Snapshot, assertions: &[StateAssertion]) -> (bool, Vec<String>) {
    let mut failures = Vec::new();

    for assertion in assertions {
        if !evaluate_assertion(snapshot, assertion) {
            failures.push(format!("{} ≠ {}", assertion.query, assertion.expected));
        }
    }

    (failures.is_empty(), failures)
}

/// Evaluate a single state assertion against a snapshot.
///
/// Supported query patterns (P0):
/// - `entity_alive(id)` — entity exists
/// - `entity.field` — simple property check (location, etc.)
/// - `knows(entity, secret)` — secret.known_by contains entity
fn evaluate_assertion(snapshot: &Snapshot, assertion: &StateAssertion) -> bool {
    let query = &assertion.query;
    let expected = &assertion.expected;

    // Pattern: entity_alive(id)
    if let Some(rest) = query.strip_prefix("entity_alive(")
        && let Some(id) = rest.strip_suffix(')')
    {
        let exists = snapshot.entities.iter().any(|e| e.id() == id);
        return match expected.as_str() {
            "true" => exists,
            "false" => !exists,
            _ => exists,
        };
    }

    // Pattern: knows(entity, secret)
    if let Some(rest) = query.strip_prefix("knows(")
        && let Some(inner) = rest.strip_suffix(')')
    {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let entity_id = parts[0];
            let secret_id = parts[1];
            let knows = snapshot
                .secrets
                .iter()
                .any(|s| s.id == secret_id && s.known_by.contains(&entity_id.to_string()));
            return match expected.as_str() {
                "true" => knows,
                "false" => !knows,
                _ => knows,
            };
        }
    }

    // Pattern: entity_id.field = value
    if let Some(dot_pos) = query.find('.') {
        let entity_id = &query[..dot_pos];
        let field = &query[dot_pos + 1..];

        if let Some(entity) = snapshot.entities.iter().find(|e| e.id() == entity_id) {
            let c = entity.as_character();
            return match field {
                "location" => c
                    .and_then(|c| c.location.as_deref())
                    .is_some_and(|loc| loc == expected),
                "type" => entity.entity_type() == *expected,
                "name" => entity.name().is_some_and(|n| n == expected),
                // Check if entity has a trait
                f if f.starts_with("has_trait(") => {
                    if let Some(t) = f
                        .strip_prefix("has_trait(")
                        .and_then(|r| r.strip_suffix(')'))
                    {
                        let has = c.is_some_and(|c| c.traits.contains(&t.to_string()));
                        match expected.as_str() {
                            "true" => has,
                            "false" => !has,
                            _ => has,
                        }
                    } else {
                        false
                    }
                }
                // Check if entity has a specific relationship
                f if f.starts_with("rel(") => {
                    if let Some(t) = f.strip_prefix("rel(").and_then(|r| r.strip_suffix(')')) {
                        c.is_some_and(|c| {
                            c.relationships
                                .iter()
                                .any(|r| r.target == t || r.role == t)
                        })
                    } else {
                        false
                    }
                }
                // Check if entity has item
                f if f.starts_with("has_item(") => {
                    if let Some(item) = f
                        .strip_prefix("has_item(")
                        .and_then(|r| r.strip_suffix(')'))
                    {
                        let has = c.is_some_and(|c| c.inventory.contains(&item.to_string()));
                        match expected.as_str() {
                            "true" => has,
                            "false" => !has,
                            _ => has,
                        }
                    } else {
                        false
                    }
                }
                _ => false, // Unknown field
            };
        } else {
            return false; // Entity not found
        }
    }

    false // Unknown query pattern
}

/// Check worldbuilding constraints against a snapshot.
/// Returns (passed, failed_reasons).
pub fn check_worldbuilding(
    snapshot: &Snapshot,
    counts: &Option<crate::state::phase::WorldbuildingCounts>,
    min_rel_density: Option<f32>,
) -> (bool, Vec<String>) {
    let mut failures = Vec::new();

    if let Some(counts) = counts {
        let n_chars = snapshot
            .entities
            .iter()
            .filter(|e| e.is_character())
            .count() as u32;
        let n_locs = snapshot
            .entities
            .iter()
            .filter(|e| e.is_location())
            .count() as u32;
        let n_factions = snapshot
            .entities
            .iter()
            .filter(|e| e.is_faction())
            .count() as u32;
        let n_secrets = snapshot.secrets.len() as u32;

        if let Some(min) = counts.characters {
            if n_chars < min {
                failures.push(format!("characters {n_chars}/{min}"));
            }
        }
        if let Some(min) = counts.locations {
            if n_locs < min {
                failures.push(format!("locations {n_locs}/{min}"));
            }
        }
        if let Some(min) = counts.factions {
            if n_factions < min {
                failures.push(format!("factions {n_factions}/{min}"));
            }
        }
        if let Some(min) = counts.secrets {
            if n_secrets < min {
                failures.push(format!("secrets {n_secrets}/{min}"));
            }
        }
    }

    if let Some(min_density) = min_rel_density {
        let characters: Vec<_> = snapshot
            .entities
            .iter()
            .filter_map(|e| e.as_character())
            .collect();
        let n = characters.len();
        if n > 0 {
            let total_rels: usize = characters.iter().map(|c| c.relationships.len()).sum();
            let density = total_rels as f32 / n as f32;
            if density < min_density {
                failures.push(format!("rel_density {density:.1}/{min_density}"));
            }
        }
    }

    (failures.is_empty(), failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::{Character, Entity, Relationship};
    use crate::input::secret::Secret;

    fn test_snapshot() -> Snapshot {
        Snapshot {
            chapter: "test".into(),
            entities: vec![
                Entity::Character(Character {
                    id: "kian".into(),
                    name: Some("基安".into()),
                    location: Some("oasis_gate".into()),
                    traits: vec!["brave".into()],
                    inventory: vec!["knife".into()],
                    relationships: vec![Relationship {
                        target: "nova".into(),
                        role: "hostile".into(),
                        trust: -2,
                        affinity: -2,
                        respect: 0,
                    }],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    goals: vec![],
            tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "nova".into(),
                    name: Some("诺娃".into()),
                    location: Some("oasis_gate".into()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    inventory: vec![],
                    relationships: vec![],
                    goals: vec![],
            tags: vec![],
                    description: None,
                }),
            ],
            secrets: vec![Secret {
                id: "oasis_truth".into(),
                content: "test".into(),
                known_by: vec!["kian".into()],
                revealed_to_reader: false,
                dramatic_function: None,
            }],
            goal_entities: vec![],
        }
    }

    #[test]
    fn entity_alive_pass() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "entity_alive(kian)".into(),
            expected: "true".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(ok);
    }

    #[test]
    fn entity_alive_fail() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "entity_alive(ghost)".into(),
            expected: "true".into(),
        };
        let (ok, failures) = check_assertions(&snap, &[a]);
        assert!(!ok);
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn location_check() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "kian.location".into(),
            expected: "oasis_gate".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(ok);
    }

    #[test]
    fn location_check_fail() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "kian.location".into(),
            expected: "wasteland".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(!ok);
    }

    #[test]
    fn knows_check() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "knows(kian, oasis_truth)".into(),
            expected: "true".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(ok);
    }

    #[test]
    fn knows_check_fail() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "knows(nova, oasis_truth)".into(),
            expected: "true".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(!ok);
    }

    #[test]
    fn has_trait_check() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "kian.has_trait(brave)".into(),
            expected: "true".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(ok);
    }

    #[test]
    fn has_item_check() {
        let snap = test_snapshot();
        let a = StateAssertion {
            query: "kian.has_item(knife)".into(),
            expected: "true".into(),
        };
        let (ok, _) = check_assertions(&snap, &[a]);
        assert!(ok);
    }

    #[test]
    fn worldbuilding_counts_pass() {
        let snap = test_snapshot(); // has 2 characters
        let counts = crate::state::phase::WorldbuildingCounts {
            characters: Some(2),
            locations: None,
            factions: None,
            secrets: Some(1),
        };
        let (ok, _) = check_worldbuilding(&snap, &Some(counts), None);
        assert!(ok);
    }

    #[test]
    fn worldbuilding_counts_fail() {
        let snap = test_snapshot(); // has 2 characters
        let counts = crate::state::phase::WorldbuildingCounts {
            characters: Some(5),
            locations: None,
            factions: None,
            secrets: None,
        };
        let (ok, failures) = check_worldbuilding(&snap, &Some(counts), None);
        assert!(!ok);
        assert!(failures[0].contains("characters 2/5"));
    }

    #[test]
    fn worldbuilding_rel_density() {
        let snap = test_snapshot(); // kian has 1 rel, nova has 0 → avg 0.5
        let (ok, failures) = check_worldbuilding(&snap, &None, Some(1.0));
        assert!(!ok);
        assert!(failures[0].contains("rel_density"));
    }

    #[test]
    fn worldbuilding_rel_density_pass() {
        let snap = test_snapshot(); // avg 0.5
        let (ok, _) = check_worldbuilding(&snap, &None, Some(0.5));
        assert!(ok);
    }
}
