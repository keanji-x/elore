//! Embedding clustering — discover structure from vector space.
//!
//! RAPTOR-inspired: embed chunks, cluster, examine what emerges.
//! If embeddings are good, clusters should align with narrative structure
//! (character arcs, locations, plot threads) without any extraction.

use super::chunker::Chunk;
use crate::memory::recall::cosine_similarity;

// ══════════════════════════════════════════════════════════════════
// K-means clustering
// ══════════════════════════════════════════════════════════════════

/// A cluster of chunk indices.
#[derive(Debug, Clone)]
pub struct Cluster {
    pub id: usize,
    pub centroid: Vec<f32>,
    pub members: Vec<usize>, // chunk indices
}

/// Run K-means clustering on embeddings.
pub fn kmeans(
    embeddings: &[Vec<f32>],
    k: usize,
    max_iters: usize,
) -> Vec<Cluster> {
    if embeddings.is_empty() || k == 0 {
        return Vec::new();
    }
    let k = k.min(embeddings.len());
    let dim = embeddings[0].len();

    // Init centroids: pick k evenly spaced embeddings
    let step = embeddings.len() / k;
    let mut centroids: Vec<Vec<f32>> = (0..k)
        .map(|i| embeddings[i * step].clone())
        .collect();

    let mut assignments = vec![0usize; embeddings.len()];

    for _iter in 0..max_iters {
        // Assign each embedding to nearest centroid
        let mut changed = false;
        for (i, emb) in embeddings.iter().enumerate() {
            let best = centroids
                .iter()
                .enumerate()
                .map(|(ci, c)| (ci, cosine_similarity(emb, c)))
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(ci, _)| ci)
                .unwrap_or(0);
            if assignments[i] != best {
                assignments[i] = best;
                changed = true;
            }
        }

        if !changed {
            break;
        }

        // Recompute centroids
        for ci in 0..k {
            let mut sum = vec![0.0f32; dim];
            let mut count = 0u32;
            for (i, emb) in embeddings.iter().enumerate() {
                if assignments[i] == ci {
                    for (j, v) in emb.iter().enumerate() {
                        sum[j] += v;
                    }
                    count += 1;
                }
            }
            if count > 0 {
                centroids[ci] = sum.iter().map(|v| v / count as f32).collect();
            }
        }
    }

    // Build clusters
    let mut clusters: Vec<Cluster> = (0..k)
        .map(|ci| Cluster {
            id: ci,
            centroid: centroids[ci].clone(),
            members: Vec::new(),
        })
        .collect();

    for (i, &ci) in assignments.iter().enumerate() {
        clusters[ci].members.push(i);
    }

    // Sort by size descending
    clusters.sort_by(|a, b| b.members.len().cmp(&a.members.len()));
    // Re-id after sorting
    for (i, c) in clusters.iter_mut().enumerate() {
        c.id = i;
    }

    clusters
}

// ══════════════════════════════════════════════════════════════════
// Cluster analysis
// ══════════════════════════════════════════════════════════════════

/// Analyze a cluster: extract keywords, chapter distribution, preview.
#[derive(Debug, Clone)]
pub struct ClusterProfile {
    pub id: usize,
    pub size: usize,
    /// Chapter indices this cluster spans.
    pub chapter_range: (usize, usize),
    /// Most common chapters.
    pub top_chapters: Vec<(usize, usize)>,
    /// Distinct chapters covered.
    pub chapter_count: usize,
    /// Average within-cluster similarity (cohesion).
    pub cohesion: f32,
    /// Representative text samples (closest to centroid).
    pub samples: Vec<String>,
    /// Top keywords (most frequent 2-3 char sequences in this cluster).
    pub keywords: Vec<(String, usize)>,
}

/// Profile all clusters.
pub fn profile_clusters(
    clusters: &[Cluster],
    chunks: &[Chunk],
    embeddings: &[Vec<f32>],
) -> Vec<ClusterProfile> {
    clusters
        .iter()
        .map(|c| profile_one(c, chunks, embeddings))
        .collect()
}

fn profile_one(
    cluster: &Cluster,
    chunks: &[Chunk],
    embeddings: &[Vec<f32>],
) -> ClusterProfile {
    let members = &cluster.members;

    // Chapter distribution
    let mut chapter_counts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for &idx in members {
        if let Some(chunk) = chunks.get(idx) {
            *chapter_counts.entry(chunk.chapter_index).or_default() += 1;
        }
    }
    let chapter_range = (
        chapter_counts.keys().next().copied().unwrap_or(0),
        chapter_counts.keys().next_back().copied().unwrap_or(0),
    );
    let mut top_chapters: Vec<(usize, usize)> = chapter_counts.into_iter().collect();
    top_chapters.sort_by(|a, b| b.1.cmp(&a.1));
    let chapter_count = top_chapters.len();
    top_chapters.truncate(5);

    // Cohesion: average similarity to centroid
    let cohesion = if members.is_empty() {
        0.0
    } else {
        let sum: f32 = members
            .iter()
            .filter_map(|&idx| embeddings.get(idx))
            .map(|emb| cosine_similarity(emb, &cluster.centroid))
            .sum();
        sum / members.len() as f32
    };

    // Samples: top 3 closest to centroid
    let mut scored: Vec<(usize, f32)> = members
        .iter()
        .filter_map(|&idx| {
            embeddings.get(idx).map(|emb| {
                (idx, cosine_similarity(emb, &cluster.centroid))
            })
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let samples: Vec<String> = scored
        .iter()
        .take(3)
        .filter_map(|(idx, _)| {
            chunks.get(*idx).map(|c| {
                let preview: String = c.text.chars().take(120).collect();
                format!("[ch{}] {}", c.chapter_index, preview)
            })
        })
        .collect();

    // Keywords: frequent 2-3 char sequences across cluster texts
    let keywords = extract_keywords(members, chunks, 10);

    ClusterProfile {
        id: cluster.id,
        size: members.len(),
        chapter_range,
        top_chapters,
        chapter_count,
        cohesion,
        samples,
        keywords,
    }
}

fn extract_keywords(
    member_indices: &[usize],
    chunks: &[Chunk],
    top_n: usize,
) -> Vec<(String, usize)> {
    let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for &idx in member_indices {
        if let Some(chunk) = chunks.get(idx) {
            let chars: Vec<char> = chunk.text.chars().collect();
            for window_size in [2, 3] {
                for window in chars.windows(window_size) {
                    let candidate: String = window.iter().collect();
                    if candidate.chars().all(|c| is_cjk(c)) {
                        *freq.entry(candidate).or_default() += 1;
                    }
                }
            }
        }
    }

    // Filter out very common words (appear in >50% of chunks)
    let threshold = member_indices.len() / 2;
    let mut sorted: Vec<(String, usize)> = freq
        .into_iter()
        .filter(|(_, count)| *count <= threshold && *count >= 3)
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(top_n);
    sorted
}

fn is_cjk(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)
}

/// Print a summary of cluster profiles.
pub fn print_cluster_summary(profiles: &[ClusterProfile]) {
    println!("═══════════════════════════════════════════");
    println!("     Cluster Analysis ({} clusters)", profiles.len());
    println!("═══════════════════════════════════════════");

    for p in profiles.iter().take(20) {
        println!();
        println!(
            "── Cluster {} ── {} chunks, ch{}-{} ({} chapters), cohesion={:.3}",
            p.id, p.size, p.chapter_range.0, p.chapter_range.1, p.chapter_count, p.cohesion
        );
        let kw: Vec<&str> = p.keywords.iter().map(|(k, _)| k.as_str()).collect();
        println!("   Keywords: {}", kw.join(", "));
        for s in &p.samples {
            println!("   Sample: {}…", s);
        }
    }
}
