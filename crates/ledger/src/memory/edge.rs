//! Memory hyperedge — the atomic unit of experiential memory.
//!
//! A `MemoryEdge` connects all participants, locations, secrets, and objects
//! involved in a single perceivable event. Unlike binary knowledge-graph triples,
//! a hyperedge preserves the co-occurrence structure: "these N things happened
//! together in one event."
//!
//! Edges are generated deterministically from content-node effects during
//! `elore build`, then enriched with embeddings for associative retrieval.

use serde::{Deserialize, Serialize};

use crate::effect::op::Op;

// ══════════════════════════════════════════════════════════════════
// MemoryEdge
// ══════════════════════════════════════════════════════════════════

/// A hyperedge representing a single perceivable narrative event.
///
/// Generated from one or more `Op`s on a content node. The edge connects
/// all entity/location/secret/object nodes involved, preserving the full
/// event structure in a single record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    /// Unique edge identifier (e.g. "node_id:seq").
    pub id: String,
    /// Content node where this event occurred.
    pub content_node: String,
    /// Human-readable event description (also used for embedding).
    pub description: String,
    /// Sequence number in the narrative timeline.
    pub seq: u32,

    // ── Hyperedge connections (multi-type) ──────────────────────
    /// Character IDs involved as actors or subjects.
    pub participants: Vec<String>,
    /// Location IDs where the event takes place.
    pub locations: Vec<String>,
    /// Secret IDs disclosed or relevant to this event.
    pub secrets: Vec<String>,
    /// Objects (inventory items) involved.
    pub objects: Vec<String>,

    // ── Metadata ────────────────────────────────────────────────
    /// Emotional valence: -1.0 (traumatic) to +1.0 (joyful).
    pub emotional_valence: f32,
    /// Base salience before decay: 0.0 (trivial) to 1.0 (life-changing).
    pub salience: f32,
    /// How many times this memory has been referenced/rehearsed.
    #[serde(default)]
    pub rehearsal_count: u32,
    /// The original Ops that generated this edge.
    pub source_ops: Vec<Op>,
}

// ══════════════════════════════════════════════════════════════════
// Perception
// ══════════════════════════════════════════════════════════════════

/// How a character came to know about an event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionMode {
    /// Was present and directly observed the event.
    Witnessed,
    /// Was told by a trusted source.
    Told,
    /// Logically deduced from other known facts.
    Inferred,
    /// Heard through unreliable channels.
    Rumor,
    /// Deliberately fed false information (deception).
    False,
}

impl PerceptionMode {
    /// Reliability weight for recall scoring (0.0–1.0).
    pub fn reliability(self) -> f32 {
        match self {
            Self::Witnessed => 1.0,
            Self::Told => 0.8,
            Self::Inferred => 0.6,
            Self::Rumor => 0.3,
            Self::False => 0.9, // feels real to the character
        }
    }
}

/// A character's perception of a specific memory edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Perception {
    pub character: String,
    pub edge_id: String,
    pub mode: PerceptionMode,
}

// ══════════════════════════════════════════════════════════════════
// Edge construction from Ops
// ══════════════════════════════════════════════════════════════════

impl MemoryEdge {
    /// Create a memory edge from a group of ops on a content node.
    ///
    /// Ops are grouped by content node during build. Each group becomes
    /// one hyperedge connecting all involved entities.
    pub fn from_ops(
        content_node: &str,
        seq: u32,
        ops: &[Op],
    ) -> Self {
        let mut participants = Vec::new();
        let mut locations = Vec::new();
        let mut secrets = Vec::new();
        let mut objects = Vec::new();

        for op in ops {
            match op {
                Op::AddTrait { entity, .. }
                | Op::RemoveTrait { entity, .. }
                | Op::SetBelief { entity, .. }
                | Op::AddDesire { entity, .. }
                | Op::RemoveDesire { entity, .. } => {
                    push_unique(&mut participants, entity);
                }
                Op::Move { entity, location } => {
                    push_unique(&mut participants, entity);
                    push_unique(&mut locations, location);
                }
                Op::AddItem { entity, item } | Op::RemoveItem { entity, item } => {
                    push_unique(&mut participants, entity);
                    push_unique(&mut objects, item);
                }
                Op::AddRel { entity, target, .. } | Op::RemoveRel { entity, target } => {
                    push_unique(&mut participants, entity);
                    push_unique(&mut participants, target);
                }
                Op::Reveal { secret, to } => {
                    push_unique(&mut secrets, secret);
                    push_unique(&mut participants, to);
                }
                Op::RevealToReader { secret } => {
                    push_unique(&mut secrets, secret);
                }
                Op::ResolveGoal { owner, .. }
                | Op::FailGoal { owner, .. }
                | Op::EmergeGoal { owner, .. } => {
                    push_unique(&mut participants, owner);
                }
            }
        }

        let description = Self::describe_ops(ops);
        let emotional_valence = Self::estimate_valence(ops);
        let salience = Self::estimate_salience(ops);

        Self {
            id: format!("{content_node}:{seq}"),
            content_node: content_node.to_string(),
            description,
            seq,
            participants,
            locations,
            secrets,
            objects,
            emotional_valence,
            salience,
            rehearsal_count: 0,
            source_ops: ops.to_vec(),
        }
    }

    /// Generate a natural-language summary for embedding.
    fn describe_ops(ops: &[Op]) -> String {
        let parts: Vec<String> = ops.iter().map(Self::describe_op).collect();
        parts.join("；")
    }

    fn describe_op(op: &Op) -> String {
        match op {
            Op::Move { entity, location } => format!("{entity}移动到{location}"),
            Op::AddTrait { entity, value } => format!("{entity}获得特质「{value}」"),
            Op::RemoveTrait { entity, value } => format!("{entity}失去特质「{value}」"),
            Op::AddItem { entity, item } => format!("{entity}获得{item}"),
            Op::RemoveItem { entity, item } => format!("{entity}失去{item}"),
            Op::AddRel { entity, target, rel } => {
                format!("{entity}与{target}建立关系「{rel}」")
            }
            Op::RemoveRel { entity, target } => format!("{entity}与{target}断绝关系"),
            Op::SetBelief { entity, old, new } => {
                format!("{entity}的信念从「{old}」变为「{new}」")
            }
            Op::AddDesire { entity, value } => format!("{entity}产生欲望「{value}」"),
            Op::RemoveDesire { entity, value } => format!("{entity}放弃欲望「{value}」"),
            Op::Reveal { secret, to } => format!("{to}得知秘密「{secret}」"),
            Op::RevealToReader { secret } => format!("读者得知秘密「{secret}」"),
            Op::ResolveGoal {
                owner, goal_id, ..
            } => format!("{owner}达成目标「{goal_id}」"),
            Op::FailGoal { owner, goal_id } => format!("{owner}的目标「{goal_id}」失败"),
            Op::EmergeGoal {
                owner,
                goal_id,
                want,
                ..
            } => format!("{owner}产生新目标「{goal_id}」: {want}"),
        }
    }

    /// Heuristic emotional valence from op types.
    fn estimate_valence(ops: &[Op]) -> f32 {
        let mut score = 0.0f32;
        for op in ops {
            score += match op {
                Op::FailGoal { .. } => -0.6,
                Op::RemoveRel { .. } => -0.4,
                Op::RemoveTrait { .. } => -0.2,
                Op::RemoveDesire { .. } => -0.1,
                Op::RemoveItem { .. } => -0.1,
                Op::ResolveGoal { .. } => 0.6,
                Op::AddRel { .. } => 0.3,
                Op::AddTrait { .. } => 0.2,
                Op::Reveal { .. } => 0.1, // neutral-positive (knowledge gain)
                Op::EmergeGoal { .. } => 0.1,
                _ => 0.0,
            };
        }
        score.clamp(-1.0, 1.0)
    }

    /// Heuristic salience from op types and count.
    fn estimate_salience(ops: &[Op]) -> f32 {
        let mut score = 0.0f32;
        for op in ops {
            score += match op {
                Op::Reveal { .. } | Op::RevealToReader { .. } => 0.4,
                Op::ResolveGoal { .. } | Op::FailGoal { .. } => 0.5,
                Op::RemoveRel { .. } => 0.3,
                Op::AddRel { .. } => 0.2,
                Op::Move { .. } => 0.1,
                _ => 0.05,
            };
        }
        score.clamp(0.0, 1.0)
    }

    /// All node IDs connected by this hyperedge (for graph algorithms).
    pub fn all_nodes(&self) -> Vec<&str> {
        let mut nodes: Vec<&str> = Vec::new();
        for p in &self.participants {
            nodes.push(p);
        }
        for l in &self.locations {
            nodes.push(l);
        }
        for s in &self.secrets {
            nodes.push(s);
        }
        for o in &self.objects {
            nodes.push(o);
        }
        nodes
    }

    /// Number of nodes connected (hyperedge cardinality).
    pub fn cardinality(&self) -> usize {
        self.participants.len() + self.locations.len() + self.secrets.len() + self.objects.len()
    }
}

fn push_unique(vec: &mut Vec<String>, val: &str) {
    if !vec.iter().any(|v| v == val) {
        vec.push(val.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_from_ops() {
        let ops = vec![
            Op::Move {
                entity: "zhang_san".into(),
                location: "secret_room".into(),
            },
            Op::Reveal {
                secret: "betrayal".into(),
                to: "zhang_san".into(),
            },
        ];
        let edge = MemoryEdge::from_ops("prologue", 1, &ops);
        assert_eq!(edge.participants, vec!["zhang_san"]);
        assert_eq!(edge.locations, vec!["secret_room"]);
        assert_eq!(edge.secrets, vec!["betrayal"]);
        assert!(edge.description.contains("移动"));
        assert!(edge.description.contains("秘密"));
        assert!(edge.salience > 0.0);
    }

    #[test]
    fn test_perception_reliability() {
        assert_eq!(PerceptionMode::Witnessed.reliability(), 1.0);
        assert!(PerceptionMode::Rumor.reliability() < PerceptionMode::Told.reliability());
    }

    #[test]
    fn test_valence_range() {
        // Many negative ops should clamp to -1.0
        let ops: Vec<Op> = (0..20)
            .map(|i| Op::FailGoal {
                owner: "x".into(),
                goal_id: format!("g{i}"),
            })
            .collect();
        let edge = MemoryEdge::from_ops("test", 0, &ops);
        assert!(edge.emotional_valence >= -1.0);
        assert!(edge.emotional_valence <= 1.0);
    }
}
