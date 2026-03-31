//! Entity relationship graph — the "world map" index.
//!
//! WorldGraph indexes entities by type, location, and relationships.
//! Supports sub-graph extraction for context culling (only show entities
//! relevant to the current scene).

use std::collections::HashMap;

use crate::input::entity::Entity;

// ══════════════════════════════════════════════════════════════════
// Graph types
// ══════════════════════════════════════════════════════════════════

/// Type of edge in the world graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    At,          // character → location
    Rel(String), // character → character (relationship label)
    Connected,   // location ↔ location
    Member,      // character → faction
    Rival,       // faction ↔ faction
}

/// A node in the world graph.
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub entity_type: String,
    pub name: Option<String>,
}

/// A directed edge in the world graph.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// The world relationship graph — indexes entities for subgraph queries.
#[derive(Debug, Clone)]
pub struct WorldGraph {
    pub nodes: HashMap<String, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub by_type: HashMap<String, Vec<String>>,
    pub by_location: HashMap<String, Vec<String>>,
    pub neighbors: HashMap<String, Vec<(String, EdgeKind)>>,
}

// ══════════════════════════════════════════════════════════════════
// Build
// ══════════════════════════════════════════════════════════════════

impl WorldGraph {
    /// Build the graph from a list of entities.
    pub fn build(entities: &[Entity]) -> Self {
        let mut nodes = HashMap::new();
        let mut edges = Vec::new();
        let mut by_type: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_location: HashMap<String, Vec<String>> = HashMap::new();
        let mut neighbors: HashMap<String, Vec<(String, EdgeKind)>> = HashMap::new();

        // Register all nodes
        for entity in entities {
            let id = entity.id().to_string();
            nodes.insert(
                id.clone(),
                GraphNode {
                    id: id.clone(),
                    entity_type: entity.entity_type().to_string(),
                    name: entity.name().map(|s| s.to_string()),
                },
            );
            by_type
                .entry(entity.entity_type().to_string())
                .or_default()
                .push(id);
        }

        // Build edges
        for entity in entities {
            let eid = entity.id().to_string();

            match entity {
                Entity::Character(c) => {
                    // Character at location
                    if let Some(loc) = &c.location {
                        let edge = GraphEdge {
                            from: eid.clone(),
                            to: loc.clone(),
                            kind: EdgeKind::At,
                        };
                        neighbors
                            .entry(eid.clone())
                            .or_default()
                            .push((loc.clone(), EdgeKind::At));
                        neighbors
                            .entry(loc.clone())
                            .or_default()
                            .push((eid.clone(), EdgeKind::At));
                        by_location
                            .entry(loc.clone())
                            .or_default()
                            .push(eid.clone());
                        edges.push(edge);
                    }

                    // Character relationships
                    for r in &c.relationships {
                        let kind = EdgeKind::Rel(r.role.clone());
                        edges.push(GraphEdge {
                            from: eid.clone(),
                            to: r.target.clone(),
                            kind: kind.clone(),
                        });
                        neighbors
                            .entry(eid.clone())
                            .or_default()
                            .push((r.target.clone(), kind));
                    }
                }
                Entity::Location(l) => {
                    // Location connections
                    for conn in &l.connections {
                        let edge = GraphEdge {
                            from: eid.clone(),
                            to: conn.clone(),
                            kind: EdgeKind::Connected,
                        };
                        neighbors
                            .entry(eid.clone())
                            .or_default()
                            .push((conn.clone(), EdgeKind::Connected));
                        edges.push(edge);
                    }
                }
                Entity::Faction(f) => {
                    // Faction membership
                    for member in &f.members {
                        edges.push(GraphEdge {
                            from: member.clone(),
                            to: eid.clone(),
                            kind: EdgeKind::Member,
                        });
                        neighbors
                            .entry(member.clone())
                            .or_default()
                            .push((eid.clone(), EdgeKind::Member));
                        neighbors
                            .entry(eid.clone())
                            .or_default()
                            .push((member.clone(), EdgeKind::Member));
                    }

                    // Faction rivals
                    for rival in &f.rivals {
                        edges.push(GraphEdge {
                            from: eid.clone(),
                            to: rival.clone(),
                            kind: EdgeKind::Rival,
                        });
                        neighbors
                            .entry(eid.clone())
                            .or_default()
                            .push((rival.clone(), EdgeKind::Rival));
                    }
                }
            }
        }

        Self {
            nodes,
            edges,
            by_type,
            by_location,
            neighbors,
        }
    }

    // ── Queries ──────────────────────────────────────────────────

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all neighbors of a node within `depth` hops.
    pub fn neighborhood(&self, center_ids: &[&str], max_depth: usize) -> Vec<String> {
        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for id in center_ids {
            if self.nodes.contains_key(*id) {
                visited.insert(id.to_string());
                queue.push_back((id.to_string(), 0));
            }
        }

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            if let Some(neighbors) = self.neighbors.get(&node) {
                for (neighbor, _) in neighbors {
                    if visited.insert(neighbor.clone()) {
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }
        }

        visited.into_iter().collect()
    }

    /// Extract a subgraph including only center nodes and their direct neighbors.
    pub fn subgraph(&self, center_ids: &[&str]) -> Self {
        let included = self.neighborhood(center_ids, 1);
        let included_set: std::collections::HashSet<&str> =
            included.iter().map(|s| s.as_str()).collect();

        let nodes: HashMap<String, GraphNode> = self
            .nodes
            .iter()
            .filter(|(k, _)| included_set.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let edges: Vec<GraphEdge> = self
            .edges
            .iter()
            .filter(|e| {
                included_set.contains(e.from.as_str()) && included_set.contains(e.to.as_str())
            })
            .cloned()
            .collect();

        let by_type = Self::rebuild_by_type(&nodes);
        let by_location = Self::rebuild_by_location(&edges);
        let neighbors = Self::rebuild_neighbors(&edges);

        Self {
            nodes,
            edges,
            by_type,
            by_location,
            neighbors,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn rebuild_by_type(nodes: &HashMap<String, GraphNode>) -> HashMap<String, Vec<String>> {
        let mut bt: HashMap<String, Vec<String>> = HashMap::new();
        for (id, node) in nodes {
            bt.entry(node.entity_type.clone())
                .or_default()
                .push(id.clone());
        }
        bt
    }

    fn rebuild_by_location(edges: &[GraphEdge]) -> HashMap<String, Vec<String>> {
        let mut bl: HashMap<String, Vec<String>> = HashMap::new();
        for edge in edges {
            if edge.kind == EdgeKind::At {
                bl.entry(edge.to.clone())
                    .or_default()
                    .push(edge.from.clone());
            }
        }
        bl
    }

    fn rebuild_neighbors(edges: &[GraphEdge]) -> HashMap<String, Vec<(String, EdgeKind)>> {
        let mut nb: HashMap<String, Vec<(String, EdgeKind)>> = HashMap::new();
        for edge in edges {
            nb.entry(edge.from.clone())
                .or_default()
                .push((edge.to.clone(), edge.kind.clone()));
        }
        nb
    }

    // ── Datalog ──────────────────────────────────────────────────

    /// Export graph as Datalog facts.
    pub fn to_datalog(&self) -> String {
        let mut output = String::from("% Graph index facts\n\n");
        for edge in &self.edges {
            let fact = match &edge.kind {
                EdgeKind::At => format!("at({}, {}).", edge.from, edge.to),
                EdgeKind::Rel(r) => {
                    let quoted = if r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        r.clone()
                    } else {
                        format!("\"{}\"", r.replace('"', "\\\"").replace('\'', ""))
                    };
                    format!("rel({}, {}, {quoted}).", edge.from, edge.to)
                }
                EdgeKind::Connected => format!("connected({}, {}).", edge.from, edge.to),
                EdgeKind::Member => format!("member({}, {}).", edge.from, edge.to),
                EdgeKind::Rival => format!("rival({}, {}).", edge.from, edge.to),
            };
            output.push_str(&fact);
            output.push('\n');
        }
        output
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::{Character, Entity, Location, Relationship};

    fn make_world() -> Vec<Entity> {
        vec![
            Entity::Character(Character {
                id: "kian".into(),
                name: Some("基安".into()),
                traits: vec![],
                beliefs: vec![],
                desires: vec![],
                intentions: vec![],
                location: Some("wasteland".into()),
                relationships: vec![],
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
                desires: vec![],
                intentions: vec![],
                location: Some("oasis".into()),
                relationships: vec![Relationship {
                    target: "kian".into(),
                    role: "enemy".into(),
                    trust: -3,
                    affinity: -3,
                    respect: 0,
                }],
                inventory: vec![],
                goals: vec![],
            tags: vec![],
                description: None,
            }),
            Entity::Location(Location {
                id: "wasteland".into(),
                name: Some("废土".into()),
                properties: vec![],
                connections: vec!["oasis".into()],
                tags: vec![],
                description: None,
            }),
            Entity::Location(Location {
                id: "oasis".into(),
                name: Some("绿洲".into()),
                properties: vec![],
                connections: vec![],
                tags: vec![],
                description: None,
            }),
        ]
    }

    #[test]
    fn build_graph_counts() {
        let graph = WorldGraph::build(&make_world());
        assert_eq!(graph.node_count(), 4);
        assert!(graph.edge_count() > 0);
    }

    #[test]
    fn subgraph_from_wasteland() {
        let graph = WorldGraph::build(&make_world());
        let sub = graph.subgraph(&["wasteland"]);
        // wasteland + kian (at wasteland) + oasis (connected to wasteland)
        assert!(sub.nodes.contains_key("wasteland"));
        assert!(sub.nodes.contains_key("kian"));
        assert!(sub.nodes.contains_key("oasis"));
    }

    #[test]
    fn neighborhood_depth_0() {
        let graph = WorldGraph::build(&make_world());
        let n = graph.neighborhood(&["kian"], 0);
        assert_eq!(n, vec!["kian"]);
    }
}
