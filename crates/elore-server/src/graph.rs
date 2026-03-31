//! Snapshot → Graph translation for the frontend.

use serde::Serialize;

use ledger::input::entity::Entity;
use ledger::state::snapshot::Snapshot;

// ══════════════════════════════════════════════════════════════════
// API types
// ══════════════════════════════════════════════════════════════════

#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub secrets: Vec<SecretNode>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub beliefs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub desires: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<GoalBrief>,
}

#[derive(Serialize)]
pub struct GoalBrief {
    pub id: String,
    pub want: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<String>,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respect: Option<i8>,
}

#[derive(Serialize)]
pub struct SecretNode {
    pub id: String,
    pub content: String,
    pub known_by: Vec<String>,
    pub revealed_to_reader: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dramatic_function: Option<String>,
}

// ══════════════════════════════════════════════════════════════════
// Builder
// ══════════════════════════════════════════════════════════════════

pub fn build_graph(snapshot: &Snapshot) -> GraphResponse {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for entity in &snapshot.entities {
        match entity {
            Entity::Character(c) => {
                let goals: Vec<GoalBrief> = c
                    .goals
                    .iter()
                    .map(|g| GoalBrief {
                        id: g.id.clone(),
                        want: g.want.clone(),
                        status: format!("{:?}", g.status).to_lowercase(),
                        conflicts_with: g.conflicts_with.clone(),
                    })
                    .collect();

                nodes.push(GraphNode {
                    id: c.id.clone(),
                    node_type: "character".into(),
                    name: c.name.clone().unwrap_or_else(|| c.id.clone()),
                    traits: c.traits.clone(),
                    beliefs: c.beliefs.clone(),
                    desires: c.desires.clone(),
                    location: c.location.clone(),
                    properties: vec![],
                    alignment: None,
                    members: vec![],
                    goals,
                });

                for r in &c.relationships {
                    edges.push(GraphEdge {
                        source: c.id.clone(),
                        target: r.target.clone(),
                        edge_type: "relationship".into(),
                        label: Some(r.role.clone()),
                        trust: Some(r.trust),
                        affinity: Some(r.affinity),
                        respect: Some(r.respect),
                    });
                }

                if let Some(loc) = &c.location {
                    edges.push(GraphEdge {
                        source: c.id.clone(),
                        target: loc.clone(),
                        edge_type: "location".into(),
                        label: None,
                        trust: None,
                        affinity: None,
                        respect: None,
                    });
                }
            }
            Entity::Location(l) => {
                nodes.push(GraphNode {
                    id: l.id.clone(),
                    node_type: "location".into(),
                    name: l.name.clone().unwrap_or_else(|| l.id.clone()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    location: None,
                    properties: l.properties.clone(),
                    alignment: None,
                    members: vec![],
                    goals: vec![],
                });

                for conn in &l.connections {
                    if l.id < *conn {
                        edges.push(GraphEdge {
                            source: l.id.clone(),
                            target: conn.clone(),
                            edge_type: "connection".into(),
                            label: None,
                            trust: None,
                            affinity: None,
                            respect: None,
                        });
                    }
                }
            }
            Entity::Faction(f) => {
                nodes.push(GraphNode {
                    id: f.id.clone(),
                    node_type: "faction".into(),
                    name: f.name.clone().unwrap_or_else(|| f.id.clone()),
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    location: None,
                    properties: vec![],
                    alignment: f.alignment.clone(),
                    members: f.members.clone(),
                    goals: vec![],
                });

                for m in &f.members {
                    edges.push(GraphEdge {
                        source: m.clone(),
                        target: f.id.clone(),
                        edge_type: "membership".into(),
                        label: None,
                        trust: None,
                        affinity: None,
                        respect: None,
                    });
                }

                for r in &f.rivals {
                    if f.id < *r {
                        edges.push(GraphEdge {
                            source: f.id.clone(),
                            target: r.clone(),
                            edge_type: "rivalry".into(),
                            label: None,
                            trust: None,
                            affinity: None,
                            respect: None,
                        });
                    }
                }
            }
        }
    }

    // Goal conflict edges — use human-readable `want` text as labels
    // Build a lookup: "owner/goal_id" → goal.want
    let mut goal_wants: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for entity in &snapshot.entities {
        if let Entity::Character(c) = entity {
            for goal in &c.goals {
                goal_wants.insert(format!("{}/{}", c.id, goal.id), goal.want.clone());
            }
        }
    }

    for entity in &snapshot.entities {
        if let Entity::Character(c) = entity {
            for goal in &c.goals {
                for conflict in &goal.conflicts_with {
                    if let Some((target_owner, _)) = conflict.split_once('/') {
                        let key = format!("{}/{}", c.id, goal.id);
                        if key < *conflict {
                            let my_want = &goal.want;
                            let their_want = goal_wants
                                .get(conflict.as_str())
                                .map(|s| s.as_str())
                                .unwrap_or(conflict);
                            edges.push(GraphEdge {
                                source: c.id.clone(),
                                target: target_owner.to_string(),
                                edge_type: "goal_conflict".into(),
                                label: Some(format!("{my_want} vs {their_want}")),
                                trust: None,
                                affinity: None,
                                respect: None,
                            });
                        }
                    }
                }
            }
        }
    }

    let secrets = snapshot
        .secrets
        .iter()
        .map(|s| SecretNode {
            id: s.id.clone(),
            content: s.content.clone(),
            known_by: s.known_by.clone(),
            revealed_to_reader: s.revealed_to_reader,
            dramatic_function: s
                .dramatic_function
                .as_ref()
                .map(|f| format!("{f:?}").to_lowercase()),
        })
        .collect();

    GraphResponse {
        nodes,
        edges,
        secrets,
    }
}
