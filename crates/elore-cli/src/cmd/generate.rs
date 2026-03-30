//! `elore gen` — compile beats into a readable Markdown document.
//!
//! Traverses all phases in plan order, assembles beat texts into prose,
//! and writes a clean Markdown file (or prints to stdout).
//!
//! Output structure:
//! - Frontmatter (title, word count, phase count)
//! - Per-phase section: synopsis header + beat text in sequence
//! - Appendix: cast list from final snapshot

use std::path::Path;

use colored::Colorize;

use ledger::effect::beat::Beat;
use ledger::input::entity;
use ledger::state::phase::Phase;
use ledger::state::phase_manager::ProjectState;

/// Generate a compiled Markdown document from all beats.
///
/// `output`: path to write .md file, or None to print to stdout.
/// `phases_filter`: if non-empty, only include these phase IDs (all otherwise).
pub fn run(
    project: &Path,
    output: Option<&Path>,
    phases_filter: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let beats_dir = everlore.join("beats");
    let phases_dir = everlore.join("phases");
    let entities_dir = everlore.join("entities");

    let state = ProjectState::load(&everlore);

    // Determine which phases to include, in plan order
    let phase_ids: Vec<&String> = if phases_filter.is_empty() {
        state.plan.iter().collect()
    } else {
        state
            .plan
            .iter()
            .filter(|id| phases_filter.contains(id))
            .collect()
    };

    if phase_ids.is_empty() {
        return Err("没有找到任何 Phase，请先 `elore add phase` 并 `elore checkout`".into());
    }

    // Collect project title (use directory name)
    let title = project
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("untitled");

    let mut doc = String::new();

    // ── Gather stats first pass ───────────────────────────────────
    let mut total_words = 0u32;
    let mut total_beats = 0u32;
    let mut included_phases = 0u32;

    struct PhaseData {
        synopsis: Option<String>,
        guidance: Option<String>,
        beats: Vec<Beat>,
        words: u32,
    }

    let mut phase_data: Vec<PhaseData> = Vec::new();

    for phase_id in &phase_ids {
        let beats = Beat::load_phase(&beats_dir, phase_id);
        if beats.is_empty() {
            continue;
        }

        let phase = Phase::load(&phases_dir, phase_id).ok();
        let synopsis = phase.as_ref().and_then(|p| p.synopsis.clone());
        let guidance = phase.as_ref().and_then(|p| p.guidance.clone());
        let words = Beat::total_words(&beats);

        total_words += words;
        total_beats += beats.len() as u32;
        included_phases += 1;

        phase_data.push(PhaseData {
            synopsis,
            guidance,
            beats,
            words,
        });
    }

    if phase_data.is_empty() {
        return Err("所有 Phase 都没有 Beats，没有内容可以生成".into());
    }

    // ── Frontmatter ───────────────────────────────────────────────
    doc.push_str(&format!("# {title}\n\n"));
    doc.push_str(&format!(
        "> 共 {} 字 · {} 个 Beat · {} 个 Phase\n\n",
        total_words, total_beats, included_phases
    ));
    doc.push_str("---\n\n");

    // ── Per-phase sections ────────────────────────────────────────
    for (i, pd) in phase_data.iter().enumerate() {
        // Section header
        let section_num = i + 1;
        if let Some(ref synopsis) = pd.synopsis {
            doc.push_str(&format!("## 第{section_num}章 · {synopsis}\n\n"));
        } else {
            doc.push_str(&format!("## 第{section_num}章\n\n"));
        }

        // Optional guidance as blockquote (author's note)
        if let Some(ref guidance) = pd.guidance {
            doc.push_str(&format!("> *{guidance}*\n\n"));
        }

        // Beat texts, sorted by seq, separated by paragraph breaks
        let mut sorted_beats = pd.beats.clone();
        sorted_beats.sort_by_key(|b| b.seq);

        for beat in &sorted_beats {
            let text = beat.text.trim();
            if !text.is_empty() {
                // Each beat = one paragraph. Wrap it so it reads naturally.
                doc.push_str(text);
                doc.push_str("\n\n");
            }
        }

        // Word count footer per phase
        doc.push_str(&format!("*（本章节 {} 字）*\n\n", pd.words));
        doc.push_str("---\n\n");
    }

    // ── Appendix: Cast List ───────────────────────────────────────
    let entities = entity::load_entities(&entities_dir)?;
    let characters: Vec<_> = entities
        .iter()
        .filter(|e| e.is_character())
        .collect();
    let locations: Vec<_> = entities
        .iter()
        .filter(|e| e.is_location())
        .collect();
    let factions: Vec<_> = entities
        .iter()
        .filter(|e| e.is_faction())
        .collect();

    if !characters.is_empty() || !locations.is_empty() {
        doc.push_str("## 附录：世界设定\n\n");

        if !characters.is_empty() {
            doc.push_str("### 人物\n\n");
            for c in &characters {
                let name = c.name().unwrap_or(c.id());
                doc.push_str(&format!("- **{}** (`{}`)", name, c.id()));
                if let Some(ch) = c.as_character() {
                    if !ch.traits.is_empty() {
                        doc.push_str(&format!(" — {}", ch.traits.join("、")));
                    }
                }
                doc.push('\n');
            }
            doc.push('\n');
        }

        if !locations.is_empty() {
            doc.push_str("### 地点\n\n");
            for loc in &locations {
                let name = loc.name().unwrap_or(loc.id());
                doc.push_str(&format!("- **{}** (`{}`)\n", name, loc.id()));
            }
            doc.push('\n');
        }

        if !factions.is_empty() {
            doc.push_str("### 阵营\n\n");
            for f in &factions {
                let name = f.name().unwrap_or(f.id());
                doc.push_str(&format!("- **{}** (`{}`)\n", name, f.id()));
            }
            doc.push('\n');
        }
    }

    // ── Write or print ────────────────────────────────────────────
    let out_path = match output {
        Some(path) => path.to_path_buf(),
        None => {
            let drafts_dir = project.join("drafts");
            std::fs::create_dir_all(&drafts_dir)?;
            let mut version = 1;
            loop {
                let name = format!("《{}》-v{}.md", title, version);
                let target = drafts_dir.join(&name);
                if !target.exists() {
                    break target;
                }
                version += 1;
            }
        }
    };

    // Ensure parent directory exists (for explicit -o cases)
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&out_path, &doc)?;
    println!(
        "{} 已生成: {} ({} 字, {} beats)",
        "✓".green().bold(),
        out_path.display().to_string().cyan(),
        total_words,
        total_beats
    );

    Ok(())
}
