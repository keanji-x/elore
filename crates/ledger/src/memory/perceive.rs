//! Auto-perception engine — derive who perceived what from world state.
//!
//! Instead of manually tagging every character's knowledge, perception is
//! derived from rules applied to the snapshot at the time of the event:
//!
//! 1. **Co-location**: characters at the same location as the event witness it
//! 2. **Participation**: direct participants always perceive the event
//! 3. **Told** (deferred): derived via Datalog `could_inform` + trust threshold
//!
//! This makes memory tracking zero-maintenance — build produces perceptions
//! automatically from effects + snapshot.

use crate::input::entity::Entity;
use crate::state::snapshot::Snapshot;

use super::edge::{MemoryEdge, Perception, PerceptionMode};

// ══════════════════════════════════════════════════════════════════
// Auto-perceive
// ══════════════════════════════════════════════════════════════════

/// Derive perceptions for a memory edge from the world snapshot.
///
/// Rules:
/// 1. Characters directly involved as participants → `Witnessed`
/// 2. Characters at the same location as the event → `Witnessed`
/// 3. `Told` and `Inferred` modes are derived later via Datalog rules
pub fn auto_perceive(edge: &MemoryEdge, snapshot: &Snapshot) -> Vec<Perception> {
    let mut perceptions = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Rule 1: Direct participants always witness
    for participant in &edge.participants {
        if is_character(participant, snapshot) && seen.insert(participant.clone()) {
            perceptions.push(Perception {
                character: participant.clone(),
                edge_id: edge.id.clone(),
                mode: PerceptionMode::Witnessed,
            });
        }
    }

    // Rule 2: Characters co-located with the event
    for location in &edge.locations {
        for entity in &snapshot.entities {
            if let Entity::Character(c) = entity {
                if c.location.as_deref() == Some(location) && seen.insert(c.id.clone()) {
                    perceptions.push(Perception {
                        character: c.id.clone(),
                        edge_id: edge.id.clone(),
                        mode: PerceptionMode::Witnessed,
                    });
                }
            }
        }
    }

    // Rule 3: For Reveal ops, the recipient always perceives
    for op in &edge.source_ops {
        if let crate::effect::op::Op::Reveal { to, .. } = op {
            if seen.insert(to.clone()) {
                perceptions.push(Perception {
                    character: to.clone(),
                    edge_id: edge.id.clone(),
                    mode: PerceptionMode::Witnessed,
                });
            }
        }
    }

    perceptions
}

/// Propagate "Told" perceptions from informers to uninformed characters.
///
/// For each edge, if character A perceived it and character B didn't,
/// and A trusts B enough (trust >= threshold), B gets a `Told` perception.
///
/// This is a single propagation step — call repeatedly for multi-hop.
pub fn propagate_told(
    edge: &MemoryEdge,
    existing: &[Perception],
    snapshot: &Snapshot,
    trust_threshold: i8,
) -> Vec<Perception> {
    let informed: std::collections::HashSet<&str> = existing
        .iter()
        .filter(|p| p.edge_id == edge.id)
        .map(|p| p.character.as_str())
        .collect();

    let mut new_perceptions = Vec::new();

    for entity in &snapshot.entities {
        if let Entity::Character(c) = entity {
            if informed.contains(c.id.as_str()) {
                // This character knows — check who they'd tell
                for rel in &c.relationships {
                    if rel.trust >= trust_threshold && !informed.contains(rel.target.as_str()) {
                        // Check target is a character
                        if is_character(&rel.target, snapshot) {
                            new_perceptions.push(Perception {
                                character: rel.target.clone(),
                                edge_id: edge.id.clone(),
                                mode: PerceptionMode::Told,
                            });
                        }
                    }
                }
            }
        }
    }

    new_perceptions
}

/// Generate perceptions for all edges from a snapshot, including propagation.
pub fn perceive_all(
    edges: &[MemoryEdge],
    snapshot: &Snapshot,
    trust_threshold: i8,
    propagation_rounds: usize,
) -> Vec<Perception> {
    let mut all_perceptions: Vec<Perception> = Vec::new();

    for edge in edges {
        // Direct perception
        let mut edge_perceptions = auto_perceive(edge, snapshot);

        // Propagation rounds
        for _ in 0..propagation_rounds {
            let told = propagate_told(edge, &edge_perceptions, snapshot, trust_threshold);
            if told.is_empty() {
                break;
            }
            edge_perceptions.extend(told);
        }

        all_perceptions.extend(edge_perceptions);
    }

    all_perceptions
}

/// Check if an entity ID corresponds to a character in the snapshot.
fn is_character(id: &str, snapshot: &Snapshot) -> bool {
    snapshot
        .entities
        .iter()
        .any(|e| matches!(e, Entity::Character(c) if c.id == id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::op::Op;
    use crate::input::entity::{Character, Entity, Relationship};

    fn make_snapshot(characters: Vec<(&str, Option<&str>)>) -> Snapshot {
        Snapshot {
            position: "test".into(),
            entities: characters
                .into_iter()
                .map(|(id, loc)| {
                    Entity::Character(Character {
                        id: id.into(),
                        name: None,
                        traits: vec![],
                        beliefs: vec![],
                        desires: vec![],
                        intentions: vec![],
                        intent_targets: vec![],
                        desire_tags: vec![],
                        location: loc.map(|s| s.into()),
                        relationships: vec![],
                        inventory: vec![],
                        goals: vec![],
                        tags: vec![],
                        description: None,
                    })
                })
                .collect(),
            secrets: vec![],
            goal_entities: vec![],
        }
    }

    #[test]
    fn test_participant_perception() {
        let snapshot = make_snapshot(vec![("alice", Some("room")), ("bob", Some("elsewhere"))]);
        let ops = vec![Op::AddTrait {
            entity: "alice".into(),
            value: "brave".into(),
        }];
        let edge = MemoryEdge::from_ops("test", 1, &ops);

        let perceptions = auto_perceive(&edge, &snapshot);
        assert_eq!(perceptions.len(), 1);
        assert_eq!(perceptions[0].character, "alice");
        assert_eq!(perceptions[0].mode, PerceptionMode::Witnessed);
    }

    #[test]
    fn test_colocation_perception() {
        let snapshot = make_snapshot(vec![
            ("alice", Some("secret_room")),
            ("bob", Some("secret_room")),
            ("charlie", Some("elsewhere")),
        ]);
        let ops = vec![Op::Move {
            entity: "alice".into(),
            location: "secret_room".into(),
        }];
        let edge = MemoryEdge::from_ops("test", 1, &ops);

        let perceptions = auto_perceive(&edge, &snapshot);
        let chars: Vec<&str> = perceptions.iter().map(|p| p.character.as_str()).collect();
        assert!(chars.contains(&"alice"));
        assert!(chars.contains(&"bob")); // co-located
        assert!(!chars.contains(&"charlie")); // elsewhere
    }

    #[test]
    fn test_told_propagation() {
        let mut snapshot = make_snapshot(vec![("alice", Some("room")), ("bob", Some("elsewhere"))]);
        // Alice trusts Bob
        if let Entity::Character(ref mut c) = snapshot.entities[0] {
            c.relationships.push(Relationship {
                target: "bob".into(),
                role: "friend".into(),
                trust: 2,
                affinity: 1,
                respect: 0,
                facade_affinity: None,
                facade_respect: None,
            });
        }

        let ops = vec![Op::AddTrait {
            entity: "alice".into(),
            value: "test".into(),
        }];
        let edge = MemoryEdge::from_ops("test", 1, &ops);

        let direct = auto_perceive(&edge, &snapshot);
        assert_eq!(direct.len(), 1); // only alice

        let told = propagate_told(&edge, &direct, &snapshot, 2);
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].character, "bob");
        assert_eq!(told[0].mode, PerceptionMode::Told);
    }
}
