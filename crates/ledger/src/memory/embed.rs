//! Embedding + vector index for associative memory retrieval.
//!
//! Uses candle (pure Rust, CPU inference) to run BERT-family embedding
//! models locally. No C dependencies, no glibc issues, no network at
//! inference time.
//!
//! Model (~50MB) is downloaded once from HuggingFace Hub on first use.
//! Subsequent runs load from cache.

use std::path::Path;

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE};
use hf_hub::{Repo, RepoType, api::sync::Api};
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use super::edge::MemoryEdge;
use super::recall::cosine_similarity;

// ══════════════════════════════════════════════════════════════════
// Embedding model wrapper (candle)
// ══════════════════════════════════════════════════════════════════

/// Local text embedder using candle (pure Rust BERT inference).
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    dim: usize,
}

impl Embedder {
    /// Create an embedder with the default Chinese-optimized model.
    ///
    /// Uses `BAAI/bge-small-zh-v1.5` (~50MB, 512 dims).
    /// Downloaded once to HF cache, then loaded locally.
    pub fn new() -> Result<Self, EmbedError> {
        Self::from_repo("BAAI/bge-small-zh-v1.5")
    }

    /// Create an embedder from a HuggingFace model repo.
    ///
    /// Any BERT-family model works: BGE, MiniLM, etc.
    pub fn from_repo(model_id: &str) -> Result<Self, EmbedError> {
        let device = Device::Cpu;

        // Download model files from HF Hub (cached after first download)
        let api = Api::new().map_err(|e| EmbedError::Init(format!("HF API init: {e}")))?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .map_err(|e| EmbedError::Init(format!("download config.json: {e}")))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| EmbedError::Init(format!("download tokenizer.json: {e}")))?;
        let weights_path = repo
            .get("model.safetensors")
            .map_err(|e| EmbedError::Init(format!("download model.safetensors: {e}")))?;

        // Load config
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| EmbedError::Init(format!("read config: {e}")))?;
        let config: BertConfig = serde_json::from_str(&config_str)
            .map_err(|e| EmbedError::Init(format!("parse config: {e}")))?;
        let dim = config.hidden_size;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbedError::Init(format!("load tokenizer: {e}")))?;

        // Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .map_err(|e| EmbedError::Init(format!("load weights: {e}")))?
        };
        let model = BertModel::load(vb, &config)
            .map_err(|e| EmbedError::Init(format!("build model: {e}")))?;

        Ok(Self {
            model,
            tokenizer,
            device,
            dim,
        })
    }

    /// Embedding dimensionality.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Embed a single text string. Returns a normalized vector.
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let batch = self.embed_batch(&[text.to_string()])?;
        batch
            .into_iter()
            .next()
            .ok_or_else(|| EmbedError::Embed("empty result".into()))
    }

    /// Embed multiple texts in batch.
    ///
    /// For large batches, processes in chunks of `batch_size` to avoid OOM.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = 32;
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(batch_size) {
            let encodings = self
                .tokenizer
                .encode_batch(chunk.to_vec(), true)
                .map_err(|e| EmbedError::Embed(format!("tokenize: {e}")))?;

            let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

            let mut input_ids_vec = Vec::new();
            let mut attention_mask_vec = Vec::new();
            let mut token_type_ids_vec = Vec::new();

            for enc in &encodings {
                let ids = enc.get_ids();
                let mask = enc.get_attention_mask();
                let types = enc.get_type_ids();

                // Pad to max_len
                let mut padded_ids = ids.to_vec();
                let mut padded_mask = mask.to_vec();
                let mut padded_types = types.to_vec();
                padded_ids.resize(max_len, 0);
                padded_mask.resize(max_len, 0);
                padded_types.resize(max_len, 0);

                input_ids_vec.extend(padded_ids);
                attention_mask_vec.extend(padded_mask);
                token_type_ids_vec.extend(padded_types);
            }

            let n = chunk.len();
            let input_ids = Tensor::from_vec(input_ids_vec, (n, max_len), &self.device)
                .map_err(|e| EmbedError::Embed(format!("tensor: {e}")))?;
            let attention_mask =
                Tensor::from_vec(attention_mask_vec, (n, max_len), &self.device)
                    .map_err(|e| EmbedError::Embed(format!("tensor: {e}")))?;
            let token_type_ids =
                Tensor::from_vec(token_type_ids_vec, (n, max_len), &self.device)
                    .map_err(|e| EmbedError::Embed(format!("tensor: {e}")))?;

            // Forward pass
            let output = self
                .model
                .forward(&input_ids, &token_type_ids, Some(&attention_mask))
                .map_err(|e| EmbedError::Embed(format!("forward: {e}")))?;

            // Mean pooling over token dimension, masked by attention
            let mask_f = attention_mask
                .to_dtype(candle_core::DType::F32)
                .map_err(|e| EmbedError::Embed(format!("cast: {e}")))?
                .unsqueeze(2)
                .map_err(|e| EmbedError::Embed(format!("unsqueeze: {e}")))?;

            let masked = output
                .broadcast_mul(&mask_f)
                .map_err(|e| EmbedError::Embed(format!("mul: {e}")))?;
            let summed = masked
                .sum(1)
                .map_err(|e| EmbedError::Embed(format!("sum: {e}")))?;
            let counts = mask_f
                .sum(1)
                .map_err(|e| EmbedError::Embed(format!("count: {e}")))?;
            let pooled = summed
                .broadcast_div(&counts)
                .map_err(|e| EmbedError::Embed(format!("div: {e}")))?;

            // L2 normalize
            let norms = pooled
                .sqr()
                .map_err(|e| EmbedError::Embed(format!("sqr: {e}")))?
                .sum(1)
                .map_err(|e| EmbedError::Embed(format!("norm sum: {e}")))?
                .sqrt()
                .map_err(|e| EmbedError::Embed(format!("sqrt: {e}")))?
                .unsqueeze(1)
                .map_err(|e| EmbedError::Embed(format!("unsqueeze: {e}")))?;
            let normalized = pooled
                .broadcast_div(&norms)
                .map_err(|e| EmbedError::Embed(format!("normalize: {e}")))?;

            // Extract to Vec<Vec<f32>>
            let flat: Vec<f32> = normalized
                .to_vec2()
                .map_err(|e| EmbedError::Embed(format!("to_vec: {e}")))?
                .into_iter()
                .flatten()
                .collect();

            for row in flat.chunks(self.dim) {
                all_embeddings.push(row.to_vec());
            }
        }

        Ok(all_embeddings)
    }
}

// ══════════════════════════════════════════════════════════════════
// MemoryIndex — brute-force vector search
// ══════════════════════════════════════════════════════════════════

/// A scored search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub edge_index: usize,
    pub score: f32,
}

/// Vector index over memory edge embeddings.
///
/// At novel scale (≤ tens of thousands of edges), brute-force cosine is
/// microsecond-fast. The index is serializable for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryIndex {
    pub edge_ids: Vec<String>,
    pub embeddings: Vec<Vec<f32>>,
}

impl MemoryIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, edge_id: &str, embedding: Vec<f32>) {
        self.edge_ids.push(edge_id.to_string());
        self.embeddings.push(embedding);
    }

    /// Build an index from memory edges using the given embedder.
    pub fn build(edges: &[MemoryEdge], embedder: &Embedder) -> Result<Self, EmbedError> {
        let texts: Vec<String> = edges.iter().map(|e| e.description.clone()).collect();
        let embeddings = embedder.embed_batch(&texts)?;

        let mut index = Self::new();
        for (edge, emb) in edges.iter().zip(embeddings) {
            index.add(&edge.id, emb);
        }
        Ok(index)
    }

    /// Search for the top-k most similar edges to a query embedding.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<SearchHit> {
        let mut scores: Vec<SearchHit> = self
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| SearchHit {
                edge_index: i,
                score: cosine_similarity(query, emb),
            })
            .collect();

        scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }

    /// Search by text query (embeds the query first).
    pub fn search_text(
        &self,
        query: &str,
        embedder: &Embedder,
        top_k: usize,
    ) -> Result<Vec<SearchHit>, EmbedError> {
        let query_vec = embedder.embed_one(query)?;
        Ok(self.search(&query_vec, top_k))
    }

    pub fn edge_id(&self, hit: &SearchHit) -> Option<&str> {
        self.edge_ids.get(hit.edge_index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.edge_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edge_ids.is_empty()
    }

    // ── Persistence ─────────────────────────────────────────────

    pub fn save(&self, path: &Path) -> Result<(), EmbedError> {
        let json = serde_json::to_string(self).map_err(|e| EmbedError::Io(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| EmbedError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, EmbedError> {
        let json = std::fs::read_to_string(path).map_err(|e| EmbedError::Io(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| EmbedError::Io(e.to_string()))
    }
}

// ══════════════════════════════════════════════════════════════════
// Error
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("embedding model init failed: {0}")]
    Init(String),
    #[error("embedding failed: {0}")]
    Embed(String),
    #[error("io error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_index_search() {
        let mut index = MemoryIndex::new();
        index.add("e1", vec![1.0, 0.0, 0.0]);
        index.add("e2", vec![0.0, 1.0, 0.0]);
        index.add("e3", vec![0.9, 0.1, 0.0]);

        let hits = index.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(index.edge_id(&hits[0]), Some("e1"));
        assert_eq!(index.edge_id(&hits[1]), Some("e3"));
    }

    #[test]
    fn test_empty_index() {
        let index = MemoryIndex::new();
        let hits = index.search(&[1.0, 0.0], 5);
        assert!(hits.is_empty());
    }
}
