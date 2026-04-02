//! Novel loader — parse raw text into structured chapters.
//!
//! Supports common Chinese novel formats:
//! - 第X章/回/节 patterns
//! - "Chapter N" English patterns
//! - Plain separator (blank lines or "---")
//!
//! Input: a single .txt file (UTF-8).
//! Output: `Novel` with ordered `Chapter` structs.

use std::path::Path;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// A parsed novel.
#[derive(Debug, Clone)]
pub struct Novel {
    pub title: String,
    pub chapters: Vec<Chapter>,
    pub total_chars: usize,
}

/// A single chapter.
#[derive(Debug, Clone)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub paragraphs: Vec<String>,
    pub char_count: usize,
}

/// A paragraph with its chapter context.
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub chapter_index: usize,
    pub paragraph_index: usize,
    pub text: String,
}

impl Novel {
    /// Iterate all paragraphs with chapter context.
    pub fn paragraphs(&self) -> Vec<Paragraph> {
        let mut result = Vec::new();
        for ch in &self.chapters {
            for (pi, text) in ch.paragraphs.iter().enumerate() {
                result.push(Paragraph {
                    chapter_index: ch.index,
                    paragraph_index: pi,
                    text: text.clone(),
                });
            }
        }
        result
    }

    pub fn chapter_count(&self) -> usize {
        self.chapters.len()
    }
}

// ══════════════════════════════════════════════════════════════════
// Loading
// ══════════════════════════════════════════════════════════════════

/// Load a novel from a text file.
pub fn load_novel(path: &Path, title: &str) -> Result<Novel, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(parse_novel(&text, title))
}

/// Parse novel text into chapters.
pub fn parse_novel(text: &str, title: &str) -> Novel {
    let chapters = split_chapters(text);
    let total_chars = chapters.iter().map(|c| c.char_count).sum();
    Novel {
        title: title.to_string(),
        chapters,
        total_chars,
    }
}

/// Split text into chapters using heading detection.
fn split_chapters(text: &str) -> Vec<Chapter> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chapter_starts: Vec<(usize, String)> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(title) = detect_chapter_heading(trimmed) {
            chapter_starts.push((i, title));
        }
    }

    // If no chapter headings found, treat the whole text as one chapter
    if chapter_starts.is_empty() {
        let paragraphs = split_paragraphs(&lines);
        let char_count = paragraphs.iter().map(|p| p.chars().count()).sum();
        return vec![Chapter {
            index: 0,
            title: "全文".to_string(),
            paragraphs,
            char_count,
        }];
    }

    let mut chapters = Vec::new();
    for (idx, (start, title)) in chapter_starts.iter().enumerate() {
        let end = chapter_starts
            .get(idx + 1)
            .map(|(e, _)| *e)
            .unwrap_or(lines.len());
        let chapter_lines = &lines[*start + 1..end];
        let paragraphs = split_paragraphs(chapter_lines);
        let char_count = paragraphs.iter().map(|p| p.chars().count()).sum();
        chapters.push(Chapter {
            index: idx,
            title: title.clone(),
            paragraphs,
            char_count,
        });
    }

    chapters
}

/// Detect if a line is a chapter heading. Returns the heading text if so.
fn detect_chapter_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return None;
    }

    // 第X章/回/节/卷 patterns
    if let Some(rest) = trimmed.strip_prefix('第') {
        let has_marker = rest.contains('章')
            || rest.contains('回')
            || rest.contains('节')
            || rest.contains('卷');
        if has_marker {
            return Some(trimmed.to_string());
        }
    }

    // "Chapter N" or "CHAPTER N"
    let lower = trimmed.to_lowercase();
    if lower.starts_with("chapter ") {
        return Some(trimmed.to_string());
    }

    // "卷X" at start of line
    if trimmed.starts_with('卷') && trimmed.chars().count() < 30 {
        return Some(trimmed.to_string());
    }

    None
}

/// Split lines into paragraphs.
///
/// Chinese novels typically use one of:
/// 1. Blank-line separated paragraphs
/// 2. Full-width space indent (　　) per line — each line IS a paragraph
/// 3. Two-space indent per line
///
/// Strategy: each non-empty line that starts with indent is its own paragraph.
/// Lines without indent are continuation of the previous paragraph.
fn split_paragraphs(lines: &[&str]) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();

    for line in lines {
        // Strip \r if present (Windows line endings)
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }

        // Detect paragraph start: full-width space indent or 2+ space indent
        let is_indent = line.starts_with('\u{3000}')
            || line.starts_with("  ")
            || line.starts_with('\t');

        if is_indent && !current.is_empty() {
            // New indented line = new paragraph
            paragraphs.push(std::mem::take(&mut current));
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(trimmed);
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }

    paragraphs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chapter_detection() {
        assert!(detect_chapter_heading("第一章 风起云涌").is_some());
        assert!(detect_chapter_heading("第三回 鸿门宴").is_some());
        assert!(detect_chapter_heading("Chapter 1 Introduction").is_some());
        assert!(detect_chapter_heading("这是普通文本").is_none());
        assert!(detect_chapter_heading("").is_none());
    }

    #[test]
    fn test_parse_novel() {
        let text = "\
第一章 开端

这是第一章的内容。

角色出场了。

第二章 发展

故事继续发展。

高潮即将到来。
";
        let novel = parse_novel(text, "测试小说");
        assert_eq!(novel.chapter_count(), 2);
        assert_eq!(novel.chapters[0].title, "第一章 开端");
        assert_eq!(novel.chapters[1].title, "第二章 发展");
        assert!(novel.chapters[0].paragraphs.len() >= 2);
    }

    #[test]
    fn test_no_chapters() {
        let text = "这是一段没有章节标记的文本。\n\n另一段话。";
        let novel = parse_novel(text, "无章节");
        assert_eq!(novel.chapter_count(), 1);
        assert_eq!(novel.chapters[0].title, "全文");
    }
}
