//! `elore ingest` — compile Markdown drafts into formal beats.
//!
//! Scans `.everlore/drafts/{phase}/` for `.md` files with optional
//! YAML frontmatter (effects list), extracts prose text, and produces:
//! - `beats/{phase}_{seq:03}.json` for each new draft
//! - Appends to `history.jsonl`
//! - Updates `state.json` counters

use std::path::Path;

use colored::Colorize;
use serde_json::Value;

use ledger::effect::beat::Beat;
use ledger::effect::history::History;
use ledger::state::phase_manager::ProjectState;
use ledger::Op;

/// Run `elore ingest`: scan drafts, compile to beats.
pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let drafts_dir = project.join("drafts");
    let beats_dir = everlore.join("beats");
    let phases_dir = everlore.join("phases");

    if !drafts_dir.exists() {
        println!(
            "{} drafts/ 目录不存在。请先在 drafts/<phase>/ 目录下创建 .md 文件",
            "⚠".yellow()
        );
        return Ok(());
    }

    std::fs::create_dir_all(&beats_dir)?;

    // Find all phase subdirectories
    let mut total_ingested = 0u32;
    let mut total_skipped = 0u32;
    let mut phase_dirs: Vec<_> = std::fs::read_dir(&drafts_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    phase_dirs.sort_by_key(|e| e.file_name());

    for phase_entry in &phase_dirs {
        let phase_id = phase_entry.file_name().to_string_lossy().to_string();

        // Verify phase exists
        if !phases_dir.join(format!("{phase_id}.yaml")).exists() {
            println!(
                "  {} 跳过 drafts/{} — phase 定义不存在",
                "⚠".yellow(),
                phase_id
            );
            continue;
        }

        // Load existing beats to know what's already ingested
        let existing_beats = Beat::load_phase(&beats_dir, &phase_id);
        let existing_count = existing_beats.len() as u32;

        // Collect and sort draft .md files
        let mut md_files: Vec<_> = std::fs::read_dir(phase_entry.path())?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "md")
            })
            .collect();
        md_files.sort_by_key(|e| e.file_name());

        if md_files.is_empty() {
            continue;
        }

        // Skip already-ingested drafts (by count)
        let new_drafts: Vec<_> = md_files.into_iter().skip(existing_count as usize).collect();

        if new_drafts.is_empty() {
            total_skipped += existing_count;
            continue;
        }

        println!(
            "{} {} — {} 个新 draft (已有 {} beats)",
            "→".cyan(),
            phase_id.cyan().bold(),
            new_drafts.len(),
            existing_count
        );

        let mut next_seq = Beat::next_seq(&beats_dir, &phase_id);

        for draft_entry in &new_drafts {
            let draft_path = draft_entry.path();
            let raw = std::fs::read_to_string(&draft_path)?;

            // Parse frontmatter + body
            let (frontmatter, body) = parse_frontmatter(&raw);

            // Extract effects from frontmatter
            let effects = parse_effects_from_frontmatter(&frontmatter);

            // Extract optional score/tags for annotation
            let score = extract_score(&frontmatter);
            let tags = extract_tags(&frontmatter);

            let text = body.trim().to_string();
            if text.is_empty() {
                println!(
                    "  {} 跳过 {} — 正文为空",
                    "⚠".yellow(),
                    draft_path.file_name().unwrap().to_string_lossy()
                );
                continue;
            }

            let word_count = Beat::count_words(&text);

            let beat = Beat {
                phase: phase_id.clone(),
                seq: next_seq,
                revises: None,
                revision: 0,
                text,
                effects: effects.clone(),
                word_count,
                created_by: "ai".to_string(),
                created_at: String::new(),
                revision_reason: None,
            };

            // Save beat
            beat.save(&beats_dir)?;

            // Append to history
            let history_entries = beat.as_history_entries();
            if !history_entries.is_empty() {
                History::append(&everlore, &history_entries)?;
            }

            // If frontmatter has score, also write annotation
            if let Some(s) = score {
                let ann = evaluator::annotation::Annotation {
                    beat: next_seq,
                    by: "ai".to_string(),
                    tags: tags.clone(),
                    score: s,
                    note: None,
                };
                let annotations_dir = everlore.join("annotations");
                std::fs::create_dir_all(&annotations_dir)?;
                evaluator::annotation::add_annotation(&annotations_dir, &phase_id, &ann)?;
            }

            println!(
                "  {} {} → beat #{} ({} 字, {} effects{})",
                "✓".green(),
                draft_path.file_name().unwrap().to_string_lossy().dimmed(),
                next_seq,
                word_count,
                effects.len(),
                if score.is_some() {
                    format!(", score: {}", score.unwrap())
                } else {
                    String::new()
                }
            );

            next_seq += 1;
            total_ingested += 1;
        }

        // Update state.json counters for this phase
        let all_beats = Beat::load_phase(&beats_dir, &phase_id);
        let total_words = Beat::total_words(&all_beats);
        let total_effects: u32 = all_beats.iter().map(|b| b.effects.len() as u32).sum();

        let mut state = ProjectState::load(&everlore);
        if let Some(entry) = state.phases.get_mut(&phase_id) {
            entry.beats = all_beats.len() as u32;
            entry.words = total_words;
            entry.effects = total_effects;
        }
        state.save(&everlore)?;
    }

    if total_ingested > 0 {
        println!(
            "\n{} 完成: {} 个 draft 已编译为 beats",
            "✓".green().bold(),
            total_ingested
        );
    } else {
        println!(
            "{} 没有新的 draft 需要处理 (已有 {} beats 跳过)",
            "•".dimmed(),
            total_skipped
        );
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Frontmatter parsing
// ══════════════════════════════════════════════════════════════════

/// Parse YAML frontmatter from a markdown file.
/// Returns (frontmatter as serde_json::Value, body text).
fn parse_frontmatter(raw: &str) -> (Value, String) {
    let trimmed = raw.trim_start();

    if !trimmed.starts_with("---") {
        return (Value::Null, raw.to_string());
    }

    // Find closing ---
    let after_open = &trimmed[3..];
    let Some(close_pos) = after_open.find("\n---") else {
        return (Value::Null, raw.to_string());
    };

    let yaml_str = &after_open[..close_pos].trim();
    let body = &after_open[close_pos + 4..]; // skip \n---

    let frontmatter: Value = serde_yaml::from_str(yaml_str).unwrap_or(Value::Null);
    (frontmatter, body.to_string())
}

/// Extract effects from frontmatter YAML. Supports both DSL strings
/// and structured objects.
fn parse_effects_from_frontmatter(fm: &Value) -> Vec<Op> {
    let Some(effects_val) = fm.get("effects") else {
        return vec![];
    };
    let Some(arr) = effects_val.as_array() else {
        return vec![];
    };

    arr.iter()
        .filter_map(|v| {
            if let Some(s) = v.as_str() {
                // DSL string like "move(kian, storm_plains)"
                Op::parse(s).ok()
            } else {
                // Structured JSON object
                serde_json::from_value(v.clone()).ok()
            }
        })
        .collect()
}

/// Extract optional score from frontmatter.
fn extract_score(fm: &Value) -> Option<u8> {
    fm.get("score")
        .and_then(|v| v.as_u64())
        .map(|v| v as u8)
}

/// Extract optional tags from frontmatter.
fn extract_tags(fm: &Value) -> Vec<String> {
    fm.get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frontmatter_basic() {
        let md = r#"---
effects:
  - move(kian, wasteland)
  - add_trait(kian, searching)
score: 5
tags: [epic, action]
---

这是正文内容。基安穿过了沙暴。
"#;
        let (fm, body) = parse_frontmatter(md);
        assert!(fm.get("effects").is_some());
        assert!(body.contains("基安穿过了沙暴"));

        let effects = parse_effects_from_frontmatter(&fm);
        assert_eq!(effects.len(), 2);

        assert_eq!(extract_score(&fm), Some(5));
        assert_eq!(extract_tags(&fm), vec!["epic", "action"]);
    }

    #[test]
    fn parse_frontmatter_no_frontmatter() {
        let md = "纯正文，没有 frontmatter。";
        let (fm, body) = parse_frontmatter(md);
        assert_eq!(fm, Value::Null);
        assert!(body.contains("纯正文"));
    }

    #[test]
    fn parse_frontmatter_effects_only() {
        let md = r#"---
effects:
  - reveal(ark_truth, kian)
---

方舟的真相被揭露了。"#;
        let (fm, body) = parse_frontmatter(md);
        let effects = parse_effects_from_frontmatter(&fm);
        assert_eq!(effects.len(), 1);
        assert!(body.contains("方舟的真相"));
        assert_eq!(extract_score(&fm), None);
    }
}
