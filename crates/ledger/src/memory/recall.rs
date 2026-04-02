//! Multi-signal recall scoring — "intuitive" memory retrieval.
//!
//! Combines five signals to simulate how a character would recall memories:
//!
//! 1. **Semantic** — embedding similarity to the current context
//! 2. **Emotional** — resonance between character's state and memory valence
//! 3. **Recency** — exponential time decay (high-salience events decay slower)
//! 4. **Social** — memories involving close relationships surface more easily
//! 5. **Rehearsal** — frequently referenced memories are stronger
//!
//! Weights are configurable per character (a paranoid character weights
//! emotional resonance higher; a rational one weights semantic similarity).

use super::edge::{MemoryEdge, Perception};

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

// ══════════════════════════════════════════════════════════════════
// Configuration
// ══════════════════════════════════════════════════════════════════

/// Weights for the multi-signal recall function.
///
/// Can be character-specific: a paranoid character might have higher
/// `w_emotional`, while a detective has higher `w_semantic`.
#[derive(Debug, Clone)]
pub struct RecallConfig {
    pub w_semantic: f32,
    pub w_emotional: f32,
    pub w_recency: f32,
    pub w_social: f32,
    pub w_rehearsal: f32,
    /// Base decay rate per sequence step (0.0–1.0).
    /// Higher = faster forgetting.
    pub base_decay: f32,
    /// Rehearsal normalization constant (how many rehearsals = max contribution).
    pub rehearsal_cap: f32,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            w_semantic: 0.35,
            w_emotional: 0.20,
            w_recency: 0.20,
            w_social: 0.15,
            w_rehearsal: 0.10,
            base_decay: 0.02,
            rehearsal_cap: 10.0,
        }
    }
}

impl RecallConfig {
    /// Preset: paranoid character — emotional memories dominate.
    pub fn paranoid() -> Self {
        Self {
            w_semantic: 0.20,
            w_emotional: 0.40,
            w_recency: 0.15,
            w_social: 0.15,
            w_rehearsal: 0.10,
            ..Self::default()
        }
    }

    /// Preset: analytical character — semantic relevance dominates.
    pub fn analytical() -> Self {
        Self {
            w_semantic: 0.45,
            w_emotional: 0.10,
            w_recency: 0.15,
            w_social: 0.15,
            w_rehearsal: 0.15,
            ..Self::default()
        }
    }

    /// Preset: sentimental character — social bonds surface memories.
    pub fn sentimental() -> Self {
        Self {
            w_semantic: 0.20,
            w_emotional: 0.25,
            w_recency: 0.10,
            w_social: 0.35,
            w_rehearsal: 0.10,
            ..Self::default()
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Character state (minimal, for recall scoring)
// ══════════════════════════════════════════════════════════════════

/// Minimal character context needed for recall scoring.
///
/// Extracted from the full `Character` struct at query time to avoid
/// coupling the memory module to the full entity model.
pub struct CharacterContext {
    pub id: String,
    /// Current emotional state: -1.0 (distressed) to 1.0 (elated).
    pub emotional_state: f32,
    /// Relationship strengths: target_id → combined strength (0.0–1.0).
    pub relationship_strengths: Vec<(String, f32)>,
}

impl CharacterContext {
    /// Get relationship strength to a specific target (0.0 if unknown).
    pub fn rel_strength(&self, target: &str) -> f32 {
        self.relationship_strengths
            .iter()
            .find(|(t, _)| t == target)
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    }
}

// ══════════════════════════════════════════════════════════════════
// Recall scoring
// ══════════════════════════════════════════════════════════════════

/// A scored recall result.
#[derive(Debug, Clone)]
pub struct RecallHit {
    pub edge_id: String,
    pub total_score: f32,
    pub semantic: f32,
    pub emotional: f32,
    pub recency: f32,
    pub social: f32,
    pub rehearsal: f32,
}

/// Score a single memory edge for recall by a character in a given context.
pub fn recall_score(
    edge: &MemoryEdge,
    perception: &Perception,
    query_embedding: &[f32],
    edge_embedding: &[f32],
    character: &CharacterContext,
    current_seq: u32,
    config: &RecallConfig,
) -> RecallHit {
    // 1. Semantic similarity
    let semantic = cosine_similarity(query_embedding, edge_embedding).max(0.0);

    // 2. Emotional resonance: character's current emotion × memory valence
    // A distressed character (-1.0) recalls negative memories (valence -1.0) more → product = 1.0
    // Cross-valence recall is weaker
    let emotional = (character.emotional_state * edge.emotional_valence)
        .abs()
        .sqrt()
        * if (character.emotional_state * edge.emotional_valence) >= 0.0 {
            1.0
        } else {
            0.3 // cross-valence: possible but weaker
        };

    // 3. Recency: exponential decay, modulated by salience
    // High-salience events decay slower (trauma, revelation, etc.)
    let age = current_seq.saturating_sub(edge.seq) as f32;
    let effective_decay = config.base_decay * (1.0 - edge.salience * 0.8);
    let recency = (-effective_decay * age).exp();

    // 4. Social proximity: strongest relationship to any participant
    let social = edge
        .participants
        .iter()
        .map(|p| character.rel_strength(p))
        .fold(0.0f32, f32::max);

    // 5. Rehearsal: logarithmic — diminishing returns
    let rehearsal = (1.0 + edge.rehearsal_count as f32).ln() / (1.0 + config.rehearsal_cap).ln();

    // Perception reliability modulates the final score
    let reliability = perception.mode.reliability();

    let total = reliability
        * (config.w_semantic * semantic
            + config.w_emotional * emotional
            + config.w_recency * recency
            + config.w_social * social
            + config.w_rehearsal * rehearsal);

    RecallHit {
        edge_id: edge.id.clone(),
        total_score: total,
        semantic,
        emotional,
        recency,
        social,
        rehearsal,
    }
}

/// Recall top-k memories for a character given a context query.
///
/// This is the main entry point for "intuitive" memory retrieval:
/// 1. Vector search narrows to semantically relevant candidates
/// 2. Perception filter removes what the character can't know
/// 3. Multi-signal scoring ranks by "what they'd actually recall"
pub fn recall_top_k(
    edges: &[(&MemoryEdge, &[f32])],
    perceptions: &[Perception],
    query_embedding: &[f32],
    character: &CharacterContext,
    current_seq: u32,
    config: &RecallConfig,
    top_k: usize,
) -> Vec<RecallHit> {
    // Build perception lookup: edge_id → Perception
    let perception_map: std::collections::HashMap<&str, &Perception> = perceptions
        .iter()
        .filter(|p| p.character == character.id)
        .map(|p| (p.edge_id.as_str(), p))
        .collect();

    let mut hits: Vec<RecallHit> = edges
        .iter()
        .filter_map(|(edge, embedding)| {
            // Only recall memories the character perceives
            let perception = perception_map.get(edge.id.as_str())?;
            Some(recall_score(
                edge,
                perception,
                query_embedding,
                embedding,
                character,
                current_seq,
                config,
            ))
        })
        .collect();

    hits.sort_by(|a, b| {
        b.total_score
            .partial_cmp(&a.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(top_k);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::op::Op;
    use crate::memory::edge::{MemoryEdge, PerceptionMode};

    fn test_edge(seq: u32, valence: f32, salience: f32) -> MemoryEdge {
        let mut edge = MemoryEdge::from_ops(
            "test",
            seq,
            &[Op::Move {
                entity: "alice".into(),
                location: "room".into(),
            }],
        );
        edge.emotional_valence = valence;
        edge.salience = salience;
        edge
    }

    fn test_character(emotional_state: f32) -> CharacterContext {
        CharacterContext {
            id: "alice".into(),
            emotional_state,
            relationship_strengths: vec![("bob".into(), 0.8)],
        }
    }

    #[test]
    fn test_recency_decay() {
        let config = RecallConfig::default();
        let edge_recent = test_edge(98, 0.0, 0.5);
        let edge_old = test_edge(10, 0.0, 0.5);
        let character = test_character(0.0);
        let emb = vec![1.0, 0.0];
        let perception = Perception {
            character: "alice".into(),
            edge_id: "test".into(),
            mode: PerceptionMode::Witnessed,
        };

        let hit_recent = recall_score(&edge_recent, &perception, &emb, &emb, &character, 100, &config);
        let hit_old = recall_score(&edge_old, &perception, &emb, &emb, &character, 100, &config);

        assert!(hit_recent.recency > hit_old.recency, "recent memory should have higher recency");
    }

    #[test]
    fn test_emotional_resonance() {
        let config = RecallConfig::default();
        let edge_negative = test_edge(90, -0.8, 0.5);
        let edge_positive = test_edge(90, 0.8, 0.5);
        let character = test_character(-0.5); // distressed
        let emb = vec![1.0, 0.0];
        let perception = Perception {
            character: "alice".into(),
            edge_id: "test".into(),
            mode: PerceptionMode::Witnessed,
        };

        let hit_neg = recall_score(&edge_negative, &perception, &emb, &emb, &character, 100, &config);
        let hit_pos = recall_score(&edge_positive, &perception, &emb, &emb, &character, 100, &config);

        assert!(
            hit_neg.emotional > hit_pos.emotional,
            "distressed character should recall negative memories more strongly"
        );
    }

    #[test]
    fn test_perception_mode_affects_score() {
        let config = RecallConfig::default();
        let edge = test_edge(90, 0.0, 0.5);
        let character = test_character(0.0);
        let emb = vec![1.0, 0.0];

        let witnessed = Perception {
            character: "alice".into(),
            edge_id: "test".into(),
            mode: PerceptionMode::Witnessed,
        };
        let rumor = Perception {
            character: "alice".into(),
            edge_id: "test".into(),
            mode: PerceptionMode::Rumor,
        };

        let hit_w = recall_score(&edge, &witnessed, &emb, &emb, &character, 100, &config);
        let hit_r = recall_score(&edge, &rumor, &emb, &emb, &character, 100, &config);

        assert!(hit_w.total_score > hit_r.total_score, "witnessed > rumor");
    }
}
