//! Memory builder — convert extracted events into MemoryEdges + HyperGraph.
//!
//! Bridges the raw text extraction (extractor.rs) with the memory system
//! (edge.rs, hypergraph.rs, perceive.rs).

use std::collections::BTreeSet;

use super::extractor::RawEvent;
use crate::memory::edge::{MemoryEdge, Perception, PerceptionMode};
use crate::memory::hypergraph::HyperGraph;

// ══════════════════════════════════════════════════════════════════
// Builder
// ══════════════════════════════════════════════════════════════════

/// Built memory state from a novel.
#[derive(Debug, Clone)]
pub struct BuiltMemory {
    pub graph: HyperGraph,
    pub edges: Vec<MemoryEdge>,
    pub perceptions: Vec<Perception>,
    pub stats: BuildStats,
}

/// Build statistics.
#[derive(Debug, Clone, Default)]
pub struct BuildStats {
    pub total_events: usize,
    pub total_edges: usize,
    pub total_perceptions: usize,
    pub unique_characters: usize,
    pub unique_locations: usize,
    pub chapters_covered: usize,
}

/// Build memory graph from raw events.
///
/// Events are converted to MemoryEdges using a synthetic Op representation
/// (since raw text doesn't have structured Ops). Perceptions are derived
/// from character co-occurrence: all characters in a paragraph "witnessed"
/// the event.
pub fn build_memory(events: &[RawEvent]) -> BuiltMemory {
    let mut graph = HyperGraph::new();
    let mut edges = Vec::new();
    let mut perceptions = Vec::new();
    let mut all_characters: BTreeSet<String> = BTreeSet::new();
    let mut all_locations: BTreeSet<String> = BTreeSet::new();
    let mut chapters: BTreeSet<usize> = BTreeSet::new();

    for event in events {
        let edge = event_to_edge(event);
        chapters.insert(event.chapter_index);

        // All characters in the paragraph "witnessed" the event
        for character in &event.characters {
            all_characters.insert(character.clone());
            perceptions.push(Perception {
                character: character.clone(),
                edge_id: edge.id.clone(),
                mode: PerceptionMode::Witnessed,
            });
        }

        for location in &event.locations {
            all_locations.insert(location.clone());
        }

        graph.insert(edge.clone());
        edges.push(edge);
    }

    let stats = BuildStats {
        total_events: events.len(),
        total_edges: edges.len(),
        total_perceptions: perceptions.len(),
        unique_characters: all_characters.len(),
        unique_locations: all_locations.len(),
        chapters_covered: chapters.len(),
    };

    BuiltMemory {
        graph,
        edges,
        perceptions,
        stats,
    }
}

/// Convert a raw event to a MemoryEdge.
///
/// Since we don't have real Ops, we create a synthetic edge directly
/// from the text extraction data.
fn event_to_edge(event: &RawEvent) -> MemoryEdge {
    let id = format!("ch{}:p{}", event.chapter_index, event.paragraph_index);

    // Estimate salience from character count and text features
    let char_count_factor = (event.characters.len() as f32 * 0.15).min(0.6);
    let has_location = if event.locations.is_empty() { 0.0 } else { 0.1 };
    let text_length_factor = (event.text.chars().count() as f32 / 500.0).min(0.3);
    let salience = (char_count_factor + has_location + text_length_factor).clamp(0.0, 1.0);

    MemoryEdge {
        id,
        content_node: format!("chapter_{}", event.chapter_index),
        description: truncate_text(&event.text, 200),
        seq: event.seq,
        participants: event.characters.iter().cloned().collect(),
        locations: event.locations.iter().cloned().collect(),
        secrets: Vec::new(),
        objects: Vec::new(),
        emotional_valence: event.valence,
        salience,
        rehearsal_count: 0,
        source_ops: Vec::new(), // no structured ops from raw text
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        text.to_string()
    } else {
        let mut s: String = chars[..max_chars].iter().collect();
        s.push_str("…");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_memory() {
        let events = vec![
            RawEvent {
                chapter_index: 0,
                paragraph_index: 0,
                characters: ["alice", "bob"].iter().map(|s| s.to_string()).collect(),
                locations: ["room"].iter().map(|s| s.to_string()).collect(),
                valence: 0.3,
                text: "Alice and Bob met in the room.".into(),
                seq: 0,
            },
            RawEvent {
                chapter_index: 0,
                paragraph_index: 1,
                characters: ["bob", "charlie"].iter().map(|s| s.to_string()).collect(),
                locations: BTreeSet::new(),
                valence: -0.2,
                text: "Bob confronted Charlie.".into(),
                seq: 1,
            },
        ];

        let built = build_memory(&events);
        assert_eq!(built.stats.total_edges, 2);
        assert_eq!(built.stats.unique_characters, 3);
        assert_eq!(built.stats.unique_locations, 1);
        assert_eq!(built.graph.edge_count(), 2);

        // Bob should witness both events
        let bob_perceptions: Vec<_> = built
            .perceptions
            .iter()
            .filter(|p| p.character == "bob")
            .collect();
        assert_eq!(bob_perceptions.len(), 2);
    }
}
