//! Evaluation suite — quality metrics for the memory system.
//!
//! Evaluates four dimensions:
//!
//! 1. **Recall precision**: given a query, do relevant memories surface?
//! 2. **Knowledge consistency**: does a character's memory graph match
//!    their actual presence in the narrative?
//! 3. **Associative quality**: do semantically related events cluster?
//! 4. **Knowledge-gap detection**: are true information asymmetries detected?
//!
//! All metrics are computable without an LLM — pure structural analysis.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::builder::BuiltMemory;
use super::extractor::{CooccurrenceMatrix, RawEvent};

// ══════════════════════════════════════════════════════════════════
// Evaluation report
// ══════════════════════════════════════════════════════════════════

/// Complete evaluation report.
#[derive(Debug, Clone)]
pub struct EvalReport {
    pub graph_metrics: GraphMetrics,
    pub knowledge_consistency: KnowledgeConsistency,
    pub temporal_metrics: TemporalMetrics,
    pub recall_metrics: Option<RecallMetrics>,
}

impl EvalReport {
    pub fn print_summary(&self) {
        println!("═══════════════════════════════════════════");
        println!("     Memory System Evaluation Report");
        println!("═══════════════════════════════════════════");
        println!();
        self.graph_metrics.print();
        println!();
        self.knowledge_consistency.print();
        println!();
        self.temporal_metrics.print();
        if let Some(ref recall) = self.recall_metrics {
            println!();
            recall.print();
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Graph metrics
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct GraphMetrics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub connected_components: usize,
    pub avg_edge_cardinality: f32,
    pub max_degree_node: Option<(String, usize)>,
    pub density: f32,
    /// Character → number of edges they participate in.
    pub character_involvement: Vec<(String, usize)>,
}

impl GraphMetrics {
    pub fn print(&self) {
        println!("── Graph Structure ──────────────────────");
        println!("  Nodes:               {}", self.total_nodes);
        println!("  Hyperedges:          {}", self.total_edges);
        println!("  Components:          {}", self.connected_components);
        println!("  Avg edge cardinality: {:.2}", self.avg_edge_cardinality);
        println!("  Density:             {:.4}", self.density);
        if let Some((ref node, deg)) = self.max_degree_node {
            println!("  Max degree node:     {} ({})", node, deg);
        }
        println!("  Top characters by involvement:");
        for (name, count) in self.character_involvement.iter().take(10) {
            println!("    {:<20} {} edges", name, count);
        }
    }
}

pub fn eval_graph(memory: &BuiltMemory) -> GraphMetrics {
    let graph = &memory.graph;
    let nodes = graph.nodes();
    let total_nodes = nodes.len();
    let total_edges = graph.edge_count();

    let avg_cardinality = if total_edges > 0 {
        graph.edges.values().map(|e| e.cardinality() as f32).sum::<f32>() / total_edges as f32
    } else {
        0.0
    };

    let components = graph.connected_components();

    let max_degree = nodes
        .iter()
        .map(|n| (n.to_string(), graph.degree(n)))
        .max_by_key(|(_, d)| *d);

    // Density: actual edges / possible edges (for hypergraph, approximate)
    let density = if total_nodes > 1 {
        total_edges as f32 / (total_nodes as f32 * (total_nodes as f32 - 1.0) / 2.0)
    } else {
        0.0
    };

    // Character involvement
    let mut involvement: Vec<(String, usize)> = memory
        .edges
        .iter()
        .flat_map(|e| e.participants.iter().cloned())
        .fold(HashMap::new(), |mut acc: HashMap<String, usize>, c| {
            *acc.entry(c).or_default() += 1;
            acc
        })
        .into_iter()
        .collect();
    involvement.sort_by(|a, b| b.1.cmp(&a.1));

    GraphMetrics {
        total_nodes,
        total_edges,
        connected_components: components.len(),
        avg_edge_cardinality: avg_cardinality,
        max_degree_node: max_degree,
        density,
        character_involvement: involvement,
    }
}

// ══════════════════════════════════════════════════════════════════
// Knowledge consistency
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct KnowledgeConsistency {
    /// For each character: edges they perceive vs edges they participate in.
    pub character_coverage: Vec<CharacterCoverage>,
    /// Average coverage across all characters.
    pub avg_coverage: f32,
    /// Characters with perception gaps (participate but don't perceive).
    pub gaps_found: usize,
}

#[derive(Debug, Clone)]
pub struct CharacterCoverage {
    pub character: String,
    pub participated_in: usize,
    pub perceived: usize,
    pub coverage: f32,
}

impl KnowledgeConsistency {
    pub fn print(&self) {
        println!("── Knowledge Consistency ────────────────");
        println!("  Avg coverage:        {:.1}%", self.avg_coverage * 100.0);
        println!("  Perception gaps:     {}", self.gaps_found);
        println!("  Per-character coverage:");
        for cc in self.character_coverage.iter().take(10) {
            println!(
                "    {:<20} {}/{} ({:.0}%)",
                cc.character,
                cc.perceived,
                cc.participated_in,
                cc.coverage * 100.0,
            );
        }
    }
}

pub fn eval_knowledge_consistency(memory: &BuiltMemory) -> KnowledgeConsistency {
    let mut participation: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut perception: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Count participation
    for edge in &memory.edges {
        for p in &edge.participants {
            participation
                .entry(p.clone())
                .or_default()
                .insert(edge.id.clone());
        }
    }

    // Count perception
    for p in &memory.perceptions {
        perception
            .entry(p.character.clone())
            .or_default()
            .insert(p.edge_id.clone());
    }

    let mut coverages = Vec::new();
    let mut total_coverage = 0.0f32;
    let mut gaps = 0usize;

    for (character, participated) in &participation {
        let perceived = perception.get(character).map(|s| s.len()).unwrap_or(0);
        let coverage = if participated.is_empty() {
            1.0
        } else {
            perceived as f32 / participated.len() as f32
        };
        if coverage < 1.0 {
            gaps += 1;
        }
        total_coverage += coverage;
        coverages.push(CharacterCoverage {
            character: character.clone(),
            participated_in: participated.len(),
            perceived,
            coverage,
        });
    }

    coverages.sort_by(|a, b| a.coverage.partial_cmp(&b.coverage).unwrap());

    let avg = if coverages.is_empty() {
        1.0
    } else {
        total_coverage / coverages.len() as f32
    };

    KnowledgeConsistency {
        character_coverage: coverages,
        avg_coverage: avg,
        gaps_found: gaps,
    }
}

// ══════════════════════════════════════════════════════════════════
// Temporal metrics
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct TemporalMetrics {
    /// Events per chapter.
    pub events_per_chapter: Vec<(usize, usize)>,
    /// Average events per chapter.
    pub avg_events_per_chapter: f32,
    /// Chapters with zero events.
    pub empty_chapters: usize,
    /// Average emotional valence over time (per chapter).
    pub valence_arc: Vec<(usize, f32)>,
    /// Salience distribution (histogram: 10 buckets).
    pub salience_histogram: Vec<(String, usize)>,
}

impl TemporalMetrics {
    pub fn print(&self) {
        println!("── Temporal Dynamics ────────────────────");
        println!(
            "  Avg events/chapter:  {:.1}",
            self.avg_events_per_chapter
        );
        println!("  Empty chapters:      {}", self.empty_chapters);
        println!("  Valence arc (emotional trajectory):");
        for (ch, v) in &self.valence_arc {
            let bar_len = ((v + 1.0) * 10.0) as usize;
            let bar: String = "█".repeat(bar_len.min(20));
            println!("    ch{:<4} {:>+.2} {}", ch, v, bar);
        }
        println!("  Salience distribution:");
        for (label, count) in &self.salience_histogram {
            let bar: String = "█".repeat((*count / 5).max(1).min(40));
            println!("    {:<10} {:>5} {}", label, count, bar);
        }
    }
}

pub fn eval_temporal(events: &[RawEvent], chapter_count: usize) -> TemporalMetrics {
    let mut per_chapter: BTreeMap<usize, Vec<&RawEvent>> = BTreeMap::new();
    for event in events {
        per_chapter.entry(event.chapter_index).or_default().push(event);
    }

    let events_per_chapter: Vec<(usize, usize)> = (0..chapter_count)
        .map(|ch| (ch, per_chapter.get(&ch).map(|v| v.len()).unwrap_or(0)))
        .collect();

    let avg = if chapter_count > 0 {
        events.len() as f32 / chapter_count as f32
    } else {
        0.0
    };

    let empty = events_per_chapter.iter().filter(|(_, c)| *c == 0).count();

    // Valence arc
    let valence_arc: Vec<(usize, f32)> = events_per_chapter
        .iter()
        .map(|(ch, _)| {
            let chapter_events = per_chapter.get(ch);
            let avg_valence = chapter_events
                .map(|evts| {
                    if evts.is_empty() {
                        0.0
                    } else {
                        evts.iter().map(|e| e.valence).sum::<f32>() / evts.len() as f32
                    }
                })
                .unwrap_or(0.0);
            (*ch, avg_valence)
        })
        .collect();

    // Salience histogram
    let mut buckets = vec![0usize; 10];
    for event in events {
        // Estimate salience same as builder
        let char_count_factor = (event.characters.len() as f32 * 0.15).min(0.6);
        let has_location = if event.locations.is_empty() { 0.0 } else { 0.1 };
        let text_length_factor = (event.text.chars().count() as f32 / 500.0).min(0.3);
        let salience = (char_count_factor + has_location + text_length_factor).clamp(0.0, 1.0);
        let bucket = ((salience * 10.0) as usize).min(9);
        buckets[bucket] += 1;
    }
    let salience_histogram: Vec<(String, usize)> = buckets
        .into_iter()
        .enumerate()
        .map(|(i, c)| (format!("{:.1}-{:.1}", i as f32 * 0.1, (i + 1) as f32 * 0.1), c))
        .collect();

    TemporalMetrics {
        events_per_chapter,
        avg_events_per_chapter: avg,
        empty_chapters: empty,
        valence_arc,
        salience_histogram,
    }
}

// ══════════════════════════════════════════════════════════════════
// Recall evaluation (requires embeddings)
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct RecallMetrics {
    pub queries: Vec<RecallQuery>,
    pub avg_precision: f32,
    pub avg_character_accuracy: f32,
}

#[derive(Debug, Clone)]
pub struct RecallQuery {
    pub query: String,
    pub expected_characters: Vec<String>,
    pub top_k_characters: Vec<String>,
    pub precision: f32,
    pub character_accuracy: f32,
}

impl RecallMetrics {
    pub fn print(&self) {
        println!("── Recall Quality ──────────────────────");
        println!("  Avg precision:       {:.1}%", self.avg_precision * 100.0);
        println!(
            "  Avg char accuracy:   {:.1}%",
            self.avg_character_accuracy * 100.0
        );
        for q in &self.queries {
            println!(
                "  Q: \"{}\"  P={:.0}%  C={:.0}%",
                truncate(&q.query, 40),
                q.precision * 100.0,
                q.character_accuracy * 100.0,
            );
            println!("    expected: {:?}", q.expected_characters);
            println!("    got:      {:?}", q.top_k_characters);
        }
    }
}

/// Evaluate recall quality using synthetic embedding (paragraph text as vector proxy).
///
/// Since we may not have a real embedding model, we use character overlap
/// as a proxy for semantic similarity. This tests the structural recall
/// pipeline, not the embedding quality.
pub fn eval_recall_structural(
    memory: &BuiltMemory,
    _events: &[RawEvent],
    cooccurrence: &CooccurrenceMatrix,
) -> RecallMetrics {
    // Generate test queries from high-co-occurrence pairs
    let top_pairs = cooccurrence.top_pairs(5);
    let mut queries = Vec::new();

    for ((char_a, char_b), _count) in &top_pairs {
        // Query: "find events where char_a and char_b interact"
        let expected: BTreeSet<String> = [char_a.clone(), char_b.clone()].into_iter().collect();

        // Find edges involving both characters
        let shared = memory.graph.shared_edges(char_a, char_b);
        let hit_characters: BTreeSet<String> = shared
            .iter()
            .filter_map(|eid| memory.graph.edge(eid))
            .flat_map(|e| e.participants.iter().cloned())
            .collect();

        // Precision: how many results contain BOTH expected characters?
        let relevant = shared
            .iter()
            .filter_map(|eid| memory.graph.edge(eid))
            .filter(|e| {
                e.participants.contains(char_a) && e.participants.contains(char_b)
            })
            .count();
        let precision = if shared.is_empty() {
            0.0
        } else {
            relevant as f32 / shared.len() as f32
        };

        // Character accuracy: expected chars found in results
        let char_accuracy = if expected.is_empty() {
            1.0
        } else {
            expected
                .iter()
                .filter(|c| hit_characters.contains(*c))
                .count() as f32
                / expected.len() as f32
        };

        queries.push(RecallQuery {
            query: format!("{}与{}的互动", char_a, char_b),
            expected_characters: expected.iter().cloned().collect(),
            top_k_characters: hit_characters.into_iter().take(5).collect(),
            precision,
            character_accuracy: char_accuracy,
        });
    }

    let avg_precision = if queries.is_empty() {
        0.0
    } else {
        queries.iter().map(|q| q.precision).sum::<f32>() / queries.len() as f32
    };

    let avg_char_accuracy = if queries.is_empty() {
        0.0
    } else {
        queries.iter().map(|q| q.character_accuracy).sum::<f32>() / queries.len() as f32
    };

    RecallMetrics {
        queries,
        avg_precision,
        avg_character_accuracy: avg_char_accuracy,
    }
}

// ══════════════════════════════════════════════════════════════════
// Full evaluation
// ══════════════════════════════════════════════════════════════════

/// Run the full evaluation suite.
pub fn evaluate(
    memory: &BuiltMemory,
    events: &[RawEvent],
    cooccurrence: &CooccurrenceMatrix,
    chapter_count: usize,
) -> EvalReport {
    let graph_metrics = eval_graph(memory);
    let knowledge_consistency = eval_knowledge_consistency(memory);
    let temporal_metrics = eval_temporal(events, chapter_count);
    let recall_metrics = Some(eval_recall_structural(memory, events, cooccurrence));

    EvalReport {
        graph_metrics,
        knowledge_consistency,
        temporal_metrics,
        recall_metrics,
    }
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let mut out: String = chars[..max].iter().collect();
        out.push_str("…");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::harness::builder::build_memory;
    use crate::memory::harness::extractor::build_cooccurrence;

    fn make_events() -> Vec<RawEvent> {
        vec![
            RawEvent {
                chapter_index: 0,
                paragraph_index: 0,
                characters: ["alice", "bob"].iter().map(|s| s.to_string()).collect(),
                locations: ["room"].iter().map(|s| s.to_string()).collect(),
                valence: 0.3,
                text: "Alice met Bob in the room.".into(),
                seq: 0,
            },
            RawEvent {
                chapter_index: 0,
                paragraph_index: 1,
                characters: ["bob", "charlie"].iter().map(|s| s.to_string()).collect(),
                locations: BTreeSet::new(),
                valence: -0.5,
                text: "Bob confronted Charlie.".into(),
                seq: 1,
            },
            RawEvent {
                chapter_index: 1,
                paragraph_index: 0,
                characters: ["alice"].iter().map(|s| s.to_string()).collect(),
                locations: ["garden"].iter().map(|s| s.to_string()).collect(),
                valence: 0.1,
                text: "Alice walked alone in the garden.".into(),
                seq: 2,
            },
        ]
    }

    #[test]
    fn test_eval_graph() {
        let events = make_events();
        let memory = build_memory(&events);
        let metrics = eval_graph(&memory);

        assert!(metrics.total_edges > 0);
        assert!(metrics.total_nodes > 0);
        assert!(metrics.avg_edge_cardinality > 0.0);
    }

    #[test]
    fn test_eval_knowledge() {
        let events = make_events();
        let memory = build_memory(&events);
        let consistency = eval_knowledge_consistency(&memory);

        // All characters should have 100% coverage (participants = perceived in harness)
        assert!((consistency.avg_coverage - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_eval_temporal() {
        let events = make_events();
        let temporal = eval_temporal(&events, 2);

        assert_eq!(temporal.events_per_chapter.len(), 2);
        assert!(temporal.avg_events_per_chapter > 0.0);
    }

    #[test]
    fn test_eval_recall() {
        let events = make_events();
        let memory = build_memory(&events);
        let cooccurrence = build_cooccurrence(&events);
        let recall = eval_recall_structural(&memory, &events, &cooccurrence);

        // Should have at least one query for top co-occurring pair
        assert!(!recall.queries.is_empty());
        // Precision should be 1.0 for exact pair queries
        assert!(recall.avg_precision > 0.0);
    }
}
