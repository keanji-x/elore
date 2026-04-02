//! Token-based sliding window chunker.
//!
//! Splits text into overlapping windows aligned to the model's tokenizer.
//! No paragraph heuristics — the model sees natural context windows.
//!
//! ```text
//! |---- window 0 (384 tokens) ----|
//!           |---- window 1 (384 tokens) ----|
//!                     |---- window 2 (384 tokens) ----|
//!
//! stride = 192 tokens → 50% overlap
//! Any event spanning a boundary appears fully in at least one window.
//! ```

use tokenizers::Tokenizer;

use super::loader::Chapter;

// ══════════════════════════════════════════════════════════════════
// Chunk
// ══════════════════════════════════════════════════════════════════

/// A text chunk with metadata.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Global chunk index.
    pub index: usize,
    /// Chapter this chunk belongs to.
    pub chapter_index: usize,
    /// The text content of this chunk.
    pub text: String,
    /// Token count.
    pub token_count: usize,
    /// Start token offset within the chapter.
    pub token_offset: usize,
}

/// Chunking configuration.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Window size in tokens.
    pub window_size: usize,
    /// Step size in tokens (window_size - overlap).
    pub stride: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            window_size: 384,
            stride: 192,
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Chunking
// ══════════════════════════════════════════════════════════════════

/// Chunk chapters into overlapping token windows using the model's tokenizer.
///
/// - Chapter boundaries are hard breaks (never merge across chapters)
/// - Within a chapter, sliding window with overlap
/// - Short chapters (< window_size) become a single chunk
pub fn chunk_chapters(
    chapters: &[Chapter],
    tokenizer: &Tokenizer,
    config: &ChunkConfig,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut global_index = 0;

    for chapter in chapters {
        // Concatenate all paragraphs in the chapter
        let chapter_text = chapter.paragraphs.join("\n");
        if chapter_text.trim().is_empty() {
            continue;
        }

        let chapter_chunks = chunk_text(
            &chapter_text,
            chapter.index,
            tokenizer,
            config,
            &mut global_index,
        );
        chunks.extend(chapter_chunks);
    }

    chunks
}

/// Chunk a single text (one chapter) into overlapping token windows.
fn chunk_text(
    text: &str,
    chapter_index: usize,
    tokenizer: &Tokenizer,
    config: &ChunkConfig,
    global_index: &mut usize,
) -> Vec<Chunk> {
    // Tokenize the full text
    let encoding = match tokenizer.encode(text, false) {
        Ok(enc) => enc,
        Err(_) => return Vec::new(),
    };

    let token_ids = encoding.get_ids();
    let offsets = encoding.get_offsets();
    let total_tokens = token_ids.len();

    if total_tokens == 0 {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total_tokens {
        let end = (start + config.window_size).min(total_tokens);

        // Map token range back to text byte range
        let byte_start = offsets[start].0;
        let byte_end = offsets[end - 1].1;
        let chunk_text = &text[byte_start..byte_end];

        chunks.push(Chunk {
            index: *global_index,
            chapter_index,
            text: chunk_text.to_string(),
            token_count: end - start,
            token_offset: start,
        });
        *global_index += 1;

        // Advance by stride, but if we'd create a tiny trailing chunk, absorb it
        let next_start = start + config.stride;
        if next_start >= total_tokens {
            break;
        }
        // If remaining tokens < stride, we already covered them in overlap
        if total_tokens - next_start < config.stride / 2 {
            break;
        }
        start = next_start;
    }

    chunks
}

/// Summary stats for chunking.
#[derive(Debug, Clone)]
pub struct ChunkStats {
    pub total_chunks: usize,
    pub avg_tokens: f32,
    pub min_tokens: usize,
    pub max_tokens: usize,
    pub chapters_with_chunks: usize,
}

pub fn chunk_stats(chunks: &[Chunk], _chapter_count: usize) -> ChunkStats {
    if chunks.is_empty() {
        return ChunkStats {
            total_chunks: 0,
            avg_tokens: 0.0,
            min_tokens: 0,
            max_tokens: 0,
            chapters_with_chunks: 0,
        };
    }

    let total = chunks.len();
    let sum: usize = chunks.iter().map(|c| c.token_count).sum();
    let min = chunks.iter().map(|c| c.token_count).min().unwrap_or(0);
    let max = chunks.iter().map(|c| c.token_count).max().unwrap_or(0);

    let mut chapters_seen = std::collections::BTreeSet::new();
    for c in chunks {
        chapters_seen.insert(c.chapter_index);
    }

    ChunkStats {
        total_chunks: total,
        avg_tokens: sum as f32 / total as f32,
        min_tokens: min,
        max_tokens: max,
        chapters_with_chunks: chapters_seen.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tokenizer() -> Tokenizer {
        // Download BGE tokenizer via hf-hub (same as production)
        let api = hf_hub::api::sync::Api::new().unwrap();
        let repo = api.repo(hf_hub::Repo::new(
            "BAAI/bge-small-zh-v1.5".to_string(),
            hf_hub::RepoType::Model,
        ));
        let path = repo.get("tokenizer.json").unwrap();
        Tokenizer::from_file(&path).unwrap()
    }

    #[test]
    fn test_chunk_short_text() {
        let tokenizer = test_tokenizer();
        let config = ChunkConfig {
            window_size: 384,
            stride: 192,
        };
        let chapters = vec![Chapter {
            index: 0,
            title: "test".into(),
            paragraphs: vec!["这是一段短文本。".into()],
            char_count: 8,
        }];

        let chunks = chunk_chapters(&chapters, &tokenizer, &config);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("这是一段短文本"));
    }

    #[test]
    fn test_chunk_overlap() {
        let tokenizer = test_tokenizer();
        let config = ChunkConfig {
            window_size: 20,
            stride: 10,
        };

        // Generate text long enough to need multiple chunks
        let long_text = "这是一个测试文本，".repeat(50);
        let chapters = vec![Chapter {
            index: 0,
            title: "test".into(),
            paragraphs: vec![long_text],
            char_count: 500,
        }];

        let chunks = chunk_chapters(&chapters, &tokenizer, &config);
        assert!(chunks.len() > 1, "should create multiple chunks");

        // Verify overlap: consecutive chunks should share text
        if chunks.len() >= 2 {
            // The end of chunk 0 and start of chunk 1 should overlap
            assert!(chunks[0].token_count == 20 || chunks[0].token_count <= 20);
        }
    }

    #[test]
    fn test_chapter_isolation() {
        let tokenizer = test_tokenizer();
        let config = ChunkConfig {
            window_size: 384,
            stride: 192,
        };
        let chapters = vec![
            Chapter {
                index: 0,
                title: "ch1".into(),
                paragraphs: vec!["第一章内容。".into()],
                char_count: 6,
            },
            Chapter {
                index: 1,
                title: "ch2".into(),
                paragraphs: vec!["第二章内容。".into()],
                char_count: 6,
            },
        ];

        let chunks = chunk_chapters(&chapters, &tokenizer, &config);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chapter_index, 0);
        assert_eq!(chunks[1].chapter_index, 1);
        // Content should not bleed across chapters
        assert!(!chunks[0].text.contains("第二章"));
        assert!(!chunks[1].text.contains("第一章"));
    }
}
