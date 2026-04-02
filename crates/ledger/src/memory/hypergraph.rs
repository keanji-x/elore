//! Hypergraph — structural index over memory edges.
//!
//! Unlike a regular graph where edges connect exactly 2 nodes, each hyperedge
//! connects an arbitrary set of nodes. This module provides:
//!
//! - **Incidence queries**: edges incident to a node, nodes in an edge
//! - **Intersection**: shared edges between two nodes (shared memories)
//! - **Node degree**: weighted by edge salience
//! - **Connected components**: independent knowledge domains
//! - **Subgraph extraction**: character-scoped memory views

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::edge::MemoryEdge;

// ══════════════════════════════════════════════════════════════════
// HyperGraph
// ══════════════════════════════════════════════════════════════════

/// A hypergraph indexing memory edges by their connected nodes.
///
/// Maintains a bidirectional incidence structure:
/// - `node → [edge_ids]` (which edges touch this node)
/// - `edge_id → MemoryEdge` (the edge data)
#[derive(Debug, Clone, Default)]
pub struct HyperGraph {
    /// All memory edges, keyed by edge id.
    pub edges: BTreeMap<String, MemoryEdge>,
    /// Incidence index: node_id → set of edge_ids incident to that node.
    incidence: BTreeMap<String, BTreeSet<String>>,
}

impl HyperGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a memory edge and update the incidence index.
    pub fn insert(&mut self, edge: MemoryEdge) {
        let edge_id = edge.id.clone();
        for node in edge.all_nodes() {
            self.incidence
                .entry(node.to_string())
                .or_default()
                .insert(edge_id.clone());
        }
        self.edges.insert(edge_id, edge);
    }

    /// All edge IDs incident to a given node.
    pub fn edges_of(&self, node: &str) -> Vec<&str> {
        self.incidence
            .get(node)
            .map(|set| set.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get edge by ID.
    pub fn edge(&self, id: &str) -> Option<&MemoryEdge> {
        self.edges.get(id)
    }

    /// All node IDs in the graph.
    pub fn nodes(&self) -> Vec<&str> {
        self.incidence.keys().map(|s| s.as_str()).collect()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ── Intersection ────────────────────────────────────────────

    /// Edges shared between two nodes (shared memories / co-occurrence).
    pub fn shared_edges(&self, a: &str, b: &str) -> Vec<&str> {
        let ea = self.incidence.get(a);
        let eb = self.incidence.get(b);
        match (ea, eb) {
            (Some(sa), Some(sb)) => sa.intersection(sb).map(|s| s.as_str()).collect(),
            _ => Vec::new(),
        }
    }

    /// Edges involving ALL of the given nodes (full intersection).
    pub fn edges_involving_all(&self, nodes: &[&str]) -> Vec<&str> {
        if nodes.is_empty() {
            return Vec::new();
        }
        let sets: Vec<&BTreeSet<String>> = nodes
            .iter()
            .filter_map(|n| self.incidence.get(*n))
            .collect();
        if sets.len() != nodes.len() {
            return Vec::new(); // some node has no edges
        }
        let mut result: BTreeSet<&str> = sets[0].iter().map(|s| s.as_str()).collect();
        for set in &sets[1..] {
            let other: BTreeSet<&str> = set.iter().map(|s| s.as_str()).collect();
            result = result.intersection(&other).copied().collect();
        }
        result.into_iter().collect()
    }

    // ── Degree / Centrality ─────────────────────────────────────

    /// Raw degree: number of edges incident to a node.
    pub fn degree(&self, node: &str) -> usize {
        self.incidence.get(node).map(|s| s.len()).unwrap_or(0)
    }

    /// Weighted degree: sum of edge salience for all incident edges.
    pub fn weighted_degree(&self, node: &str) -> f32 {
        self.edges_of(node)
            .iter()
            .filter_map(|eid| self.edges.get(*eid))
            .map(|e| e.salience)
            .sum()
    }

    /// Edge centrality: how "important" a hyperedge is (by cardinality × salience).
    pub fn edge_centrality(&self, edge_id: &str) -> f32 {
        self.edges
            .get(edge_id)
            .map(|e| e.cardinality() as f32 * e.salience)
            .unwrap_or(0.0)
    }

    // ── Connected Components ────────────────────────────────────

    /// Find connected components (independent knowledge domains).
    ///
    /// Two nodes are connected if they share at least one hyperedge.
    /// Returns groups of node IDs.
    pub fn connected_components(&self) -> Vec<Vec<String>> {
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut components = Vec::new();

        for node in self.incidence.keys() {
            if visited.contains(node.as_str()) {
                continue;
            }
            let mut component = Vec::new();
            let mut queue: VecDeque<&str> = VecDeque::new();
            queue.push_back(node);
            visited.insert(node);

            while let Some(current) = queue.pop_front() {
                component.push(current.to_string());
                // Find all nodes reachable via shared hyperedges
                for edge_id in self.edges_of(current) {
                    if let Some(edge) = self.edges.get(edge_id) {
                        for neighbor in edge.all_nodes() {
                            if !visited.contains(neighbor) {
                                visited.insert(neighbor);
                                queue.push_back(neighbor);
                            }
                        }
                    }
                }
            }

            component.sort();
            components.push(component);
        }

        components
    }

    // ── Subgraph extraction ─────────────────────────────────────

    /// Extract a subgraph containing only edges involving a specific node.
    /// Useful for character-scoped memory views.
    pub fn subgraph_for(&self, node: &str) -> HyperGraph {
        let mut sub = HyperGraph::new();
        for edge_id in self.edges_of(node) {
            if let Some(edge) = self.edges.get(edge_id) {
                sub.insert(edge.clone());
            }
        }
        sub
    }

    /// Extract a subgraph containing only edges with IDs in the given set.
    pub fn subgraph_by_edges(&self, edge_ids: &[&str]) -> HyperGraph {
        let mut sub = HyperGraph::new();
        for eid in edge_ids {
            if let Some(edge) = self.edges.get(*eid) {
                sub.insert(edge.clone());
            }
        }
        sub
    }

    // ── Neighborhood ────────────────────────────────────────────

    /// All nodes reachable from `node` within `hops` hyperedge traversals.
    pub fn neighborhood(&self, node: &str, hops: usize) -> BTreeSet<String> {
        let mut visited = BTreeSet::new();
        let mut frontier = BTreeSet::new();
        frontier.insert(node.to_string());
        visited.insert(node.to_string());

        for _ in 0..hops {
            let mut next_frontier = BTreeSet::new();
            for n in &frontier {
                for eid in self.edges_of(n) {
                    if let Some(edge) = self.edges.get(eid) {
                        for neighbor in edge.all_nodes() {
                            if visited.insert(neighbor.to_string()) {
                                next_frontier.insert(neighbor.to_string());
                            }
                        }
                    }
                }
            }
            frontier = next_frontier;
        }

        visited
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::op::Op;

    fn make_edge(id: &str, participants: &[&str], locations: &[&str]) -> MemoryEdge {
        let mut ops = Vec::new();
        for p in participants {
            ops.push(Op::AddTrait {
                entity: p.to_string(),
                value: "test".into(),
            });
        }
        for l in locations {
            if let Some(p) = participants.first() {
                ops.push(Op::Move {
                    entity: p.to_string(),
                    location: l.to_string(),
                });
            }
        }
        let mut edge = MemoryEdge::from_ops("test_node", 0, &ops);
        edge.id = id.to_string();
        edge
    }

    #[test]
    fn test_insert_and_query() {
        let mut g = HyperGraph::new();
        g.insert(make_edge("e1", &["alice", "bob"], &["room_a"]));
        g.insert(make_edge("e2", &["bob", "charlie"], &["room_b"]));

        assert_eq!(g.edge_count(), 2);
        assert_eq!(g.degree("bob"), 2);
        assert_eq!(g.degree("alice"), 1);
    }

    #[test]
    fn test_shared_edges() {
        let mut g = HyperGraph::new();
        g.insert(make_edge("e1", &["alice", "bob"], &[]));
        g.insert(make_edge("e2", &["bob", "charlie"], &[]));
        g.insert(make_edge("e3", &["alice", "bob"], &[]));

        let shared = g.shared_edges("alice", "bob");
        assert_eq!(shared.len(), 2); // e1, e3
        assert!(shared.contains(&"e1"));
        assert!(shared.contains(&"e3"));
    }

    #[test]
    fn test_connected_components() {
        let mut g = HyperGraph::new();
        // Component 1: alice-bob
        g.insert(make_edge("e1", &["alice", "bob"], &[]));
        // Component 2: charlie-dave (disconnected)
        g.insert(make_edge("e2", &["charlie", "dave"], &[]));

        let comps = g.connected_components();
        assert_eq!(comps.len(), 2);
    }

    #[test]
    fn test_neighborhood() {
        let mut g = HyperGraph::new();
        g.insert(make_edge("e1", &["alice", "bob"], &[]));
        g.insert(make_edge("e2", &["bob", "charlie"], &[]));

        let n1 = g.neighborhood("alice", 1);
        assert!(n1.contains("alice"));
        assert!(n1.contains("bob"));
        assert!(!n1.contains("charlie")); // 2 hops away

        let n2 = g.neighborhood("alice", 2);
        assert!(n2.contains("charlie"));
    }

    #[test]
    fn test_subgraph() {
        let mut g = HyperGraph::new();
        g.insert(make_edge("e1", &["alice", "bob"], &[]));
        g.insert(make_edge("e2", &["bob", "charlie"], &[]));
        g.insert(make_edge("e3", &["dave"], &[]));

        let sub = g.subgraph_for("bob");
        assert_eq!(sub.edge_count(), 2);
        assert!(sub.edges.contains_key("e1"));
        assert!(sub.edges.contains_key("e2"));
        assert!(!sub.edges.contains_key("e3"));
    }
}
