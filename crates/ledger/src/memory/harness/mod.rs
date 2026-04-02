//! Harness — load a novel, embed chunks, evaluate semantic retrieval.
//!
//! Pure vector RAG pipeline. No character extraction, no dialogue patterns,
//! no stopword lists. The embedding model handles semantics.
//!
//! ```bash
//! elore harness novel.txt --title "遮天"
//! elore harness novel.txt --title "遮天" --discover 20  # legacy mode
//! ```

pub mod builder;
pub mod chunker;
pub mod cluster;
pub mod evaluator;
pub mod extractor;
pub mod loader;

use std::path::Path;

use crate::memory::embed::{Embedder, MemoryIndex};
use tokenizers::Tokenizer;

/// Run the vector RAG harness: load → chunk → embed → index → query → report.
pub fn run(
    novel_path: &Path,
    title: &str,
    roster_path: Option<&Path>,
    _discover_threshold: Option<usize>,
) -> Result<evaluator::EvalReport, Box<dyn std::error::Error>> {
    // ── 1. Load novel ───────────────────────────────────────────
    println!("📖 Loading novel: {}", novel_path.display());
    let novel = loader::load_novel(novel_path, title)?;
    println!(
        "   {} chapters, {} chars",
        novel.chapter_count(),
        novel.total_chars
    );

    // ── 2. Init embedding model ─────────────────────────────────
    println!("🔧 Loading embedding model (BGE-small-zh, first run downloads ~50MB)...");
    let embedder = Embedder::new()?;
    println!("   Model loaded, {} dims", embedder.dim());

    // Get the tokenizer for chunking
    let tokenizer = load_tokenizer()?;

    // ── 3. Token-based sliding window chunking ──────────────────
    let config = chunker::ChunkConfig::default(); // 384 window, 192 stride
    println!(
        "📐 Chunking: {} token window, {} stride (50% overlap)...",
        config.window_size, config.stride
    );
    let chunks = chunker::chunk_chapters(&novel.chapters, &tokenizer, &config);
    let stats = chunker::chunk_stats(&chunks, novel.chapter_count());
    println!(
        "   {} chunks, avg {:.0} tokens, range [{}, {}]",
        stats.total_chunks, stats.avg_tokens, stats.min_tokens, stats.max_tokens
    );

    // ── 4. Embed all chunks ─────────────────────────────────────
    println!("🧠 Embedding {} chunks...", chunks.len());
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();

    let batch_size = 256;
    let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    let total_batches = (texts.len() + batch_size - 1) / batch_size;

    for (i, batch) in texts.chunks(batch_size).enumerate() {
        if (i + 1) % 10 == 0 || i + 1 == total_batches {
            println!("   batch {}/{}", i + 1, total_batches);
        }
        let batch_embeddings = embedder.embed_batch(&batch.to_vec())?;
        all_embeddings.extend(batch_embeddings);
    }

    // Build vector index
    let mut index = MemoryIndex::new();
    for (chunk, emb) in chunks.iter().zip(all_embeddings.iter()) {
        index.add(&format!("ch{}:{}", chunk.chapter_index, chunk.index), emb.clone());
    }
    println!("   Index built: {} vectors × {} dims", index.len(), embedder.dim());

    // ── 5. Cluster analysis ─────────────────────────────────────
    let n_clusters = 20; // start with 20 clusters
    println!("🔬 Clustering {} chunks into {} groups...", chunks.len(), n_clusters);
    let clusters = cluster::kmeans(&all_embeddings, n_clusters, 50);
    let profiles = cluster::profile_clusters(&clusters, &chunks, &all_embeddings);
    cluster::print_cluster_summary(&profiles);

    // ── 6. Test queries ─────────────────────────────────────────
    println!("🔍 Running test queries...");
    let test_queries = generate_test_queries(title);
    let mut query_results = Vec::new();

    for query in &test_queries {
        let hits = index.search_text(query, &embedder, 5)?;
        let mut result_texts: Vec<String> = Vec::new();
        for hit in &hits {
            if let Some(eid) = index.edge_id(hit) {
                let chunk_idx: usize = eid.split(':').nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if let Some(chunk) = chunks.get(chunk_idx) {
                    let preview: String = chunk.text.chars().take(80).collect();
                    result_texts.push(format!("[{:.3}] ch{}: {}…", hit.score, chunk.chapter_index, preview));
                }
            }
        }
        println!("\n   Q: \"{}\"", query);
        for r in &result_texts {
            println!("     {}", r);
        }
        query_results.push((query.clone(), hits.iter().map(|h| h.score).collect::<Vec<_>>()));
    }

    // ── 6. Build report ─────────────────────────────────────────
    println!("\n📊 Building evaluation report...");

    // Also run legacy structured eval if roster available
    let legacy_report = if let Some(rp) = roster_path {
        let roster = extractor::Roster::load(rp)?;
        let events = extractor::extract_events(&novel, &roster);
        let cooccurrence = extractor::build_cooccurrence(&events);
        let memory = builder::build_memory(&events);
        Some(evaluator::evaluate(&memory, &events, &cooccurrence, novel.chapter_count()))
    } else {
        None
    };

    // Print vector RAG summary
    println!("\n═══════════════════════════════════════════");
    println!("     Vector RAG Evaluation Report");
    println!("═══════════════════════════════════════════");
    println!();
    println!("── Corpus ──────────────────────────────");
    println!("  Title:              {}", title);
    println!("  Chapters:           {}", novel.chapter_count());
    println!("  Total chars:        {}", novel.total_chars);
    println!("  Chunks:             {}", stats.total_chunks);
    println!("  Avg tokens/chunk:   {:.0}", stats.avg_tokens);
    println!("  Embedding dim:      {}", embedder.dim());
    println!();
    println!("── Retrieval Quality ───────────────────");
    for (query, scores) in &query_results {
        let avg_score = if scores.is_empty() { 0.0 } else { scores.iter().sum::<f32>() / scores.len() as f32 };
        let top_score = scores.first().copied().unwrap_or(0.0);
        println!(
            "  Q: \"{}\"\n    top={:.3}  avg_top5={:.3}",
            query, top_score, avg_score
        );
    }

    // Return legacy report if available, otherwise a minimal one
    if let Some(report) = legacy_report {
        Ok(report)
    } else {
        // Minimal report (no structured analysis)
        Ok(evaluator::EvalReport {
            graph_metrics: evaluator::GraphMetrics {
                total_nodes: 0,
                total_edges: stats.total_chunks,
                connected_components: 0,
                avg_edge_cardinality: stats.avg_tokens,
                max_degree_node: None,
                density: 0.0,
                character_involvement: vec![],
            },
            knowledge_consistency: evaluator::KnowledgeConsistency {
                character_coverage: vec![],
                avg_coverage: 0.0,
                gaps_found: 0,
            },
            temporal_metrics: evaluator::eval_temporal(&[], novel.chapter_count()),
            recall_metrics: None,
        })
    }
}

/// Load the BGE tokenizer for chunking.
fn load_tokenizer() -> Result<Tokenizer, Box<dyn std::error::Error>> {
    let api = hf_hub::api::sync::Api::new()?;
    let repo = api.repo(hf_hub::Repo::new(
        "BAAI/bge-small-zh-v1.5".to_string(),
        hf_hub::RepoType::Model,
    ));
    let tokenizer_path = repo.get("tokenizer.json")?;
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| format!("load tokenizer: {e}"))?;
    Ok(tokenizer)
}

/// Generate test queries based on novel title.
fn generate_test_queries(title: &str) -> Vec<String> {
    match title {
        "遮天" => vec![
            "叶凡在荒古铜棺中的经历".into(),
            "庞博和叶凡的关系".into(),
            "大黑狗的来历".into(),
            "北斗星域的修炼体系".into(),
            "叶凡的第一次战斗".into(),
            "李小曼和叶凡分手".into(),
            "九龙拉棺".into(),
            "荒古大帝的传说".into(),
        ],
        _ => vec![
            format!("{}的主要人物", title),
            format!("{}的开头", title),
            format!("{}中最激烈的冲突", title),
            format!("{}的结局", title),
        ],
    }
}
