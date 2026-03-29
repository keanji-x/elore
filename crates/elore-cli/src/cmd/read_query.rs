//! `elore read snapshot/prompt/drama/history [--format json]`
//!
//! Unified read API for AI agents. All structured outputs support `--format json`.
//! Prompt output is always plain text (markdown).

use std::path::Path;

use colored::Colorize;
use serde_json::{json, Value};

use ledger::state::snapshot::Snapshot;
use ledger::effect::history::History;
use resolver::drama;

#[derive(Debug, Clone, PartialEq)]
pub enum Format {
    Human,
    Json,
}

impl Format {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            _ => Self::Human,
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// snapshot
// ══════════════════════════════════════════════════════════════════

pub fn read_snapshot(
    project: &Path,
    chapter: &str,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");
    let snap = Snapshot::build(chapter, &entities_dir, &everlore_dir)?;

    match format {
        Format::Json => {
            let j = snapshot_to_json(&snap);
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            println!("{}", format!("═══ Snapshot: {chapter} ═══").cyan().bold());
            println!();
            println!("角色 ({}):", snap.characters().len());
            for c in snap.characters() {
                let name = c.name.as_deref().unwrap_or(&c.id);
                let loc = c.location.as_deref().unwrap_or("?");
                println!("  {} ({}) @ {}", name.bold(), c.id, loc);
                if !c.traits.is_empty() {
                    println!("    特质: {}", c.traits.join(", "));
                }
                if !c.beliefs.is_empty() {
                    println!("    信念: {}", c.beliefs.join("; "));
                }
                if !c.desires.is_empty() {
                    println!("    欲望: {}", c.desires.join("; "));
                }
                if !c.inventory.is_empty() {
                    println!("    物品: {}", c.inventory.join(", "));
                }
                if !c.relationships.is_empty() {
                    let rels: Vec<_> = c.relationships.iter()
                        .map(|r| format!("{}({})", r.rel, r.target))
                        .collect();
                    println!("    关系: {}", rels.join(", "));
                }
            }
            println!("\n地点 ({}):", snap.locations().len());
            for l in snap.locations() {
                let name = l.name.as_deref().unwrap_or(&l.id);
                println!("  {} ({})", name.bold(), l.id);
            }
            if !snap.secrets.is_empty() {
                println!("\n秘密 ({}):", snap.secrets.len());
                for s in &snap.secrets {
                    let known = if s.known_by.is_empty() {
                        "无人".to_string()
                    } else {
                        s.known_by.join(", ")
                    };
                    let reader = if s.revealed_to_reader { "✓" } else { "✗" };
                    println!("  {} [已知: {} | 读者: {}]", s.id.bold(), known, reader);
                }
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// prompt
// ══════════════════════════════════════════════════════════════════

pub fn read_prompt(
    project: &Path,
    chapter: &str,
    pov: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");
    let snap = Snapshot::build(chapter, &entities_dir, &everlore_dir)?;
    let mut drama_node = drama::load_drama(&everlore_dir, chapter)?;

    if let Some(p) = pov {
        drama_node.director_notes.pov = Some(p.to_string());
    }

    let history = History::load(&everlore_dir);
    let prev_summary = History::previous_chapter_summary(&history, chapter);

    let prompt = resolver::prompt::AuthorPrompt::build(
        &snap,
        &drama_node,
        None,
        prev_summary.as_deref(),
        None,
    );

    // Always plain text — this goes directly to LLM
    print!("{}", prompt.rendered);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// drama
// ══════════════════════════════════════════════════════════════════

pub fn read_drama(
    project: &Path,
    chapter: &str,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore_dir = project.join(".everlore");
    let drama_node = drama::load_drama(&everlore_dir, chapter)?;

    match format {
        Format::Json => {
            // Re-serialize the drama node to JSON
            let j = serde_json::to_value(&drama_node)?;
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            println!("{}", drama_node.render());
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// history
// ══════════════════════════════════════════════════════════════════

pub fn read_history(
    project: &Path,
    chapter_filter: Option<&str>,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore_dir = project.join(".everlore");
    let history = History::load(&everlore_dir);

    let entries: Vec<_> = if let Some(ch) = chapter_filter {
        history.entries.iter().filter(|e| e.chapter == ch).collect()
    } else {
        history.entries.iter().collect()
    };

    match format {
        Format::Json => {
            let arr: Vec<Value> = entries.iter().map(|e| json!({
                "chapter": e.chapter,
                "seq": e.seq,
                "effect": e.effect.describe(),
                "op": serde_json::to_value(&e.effect).unwrap_or(Value::Null),
            })).collect();
            println!("{}", serde_json::to_string_pretty(&arr)?);
        }
        Format::Human => {
            if entries.is_empty() {
                println!("{}", "(事件日志为空)".dimmed());
                return Ok(());
            }
            println!("{}", "═══ History ═══".cyan().bold());
            for entry in &entries {
                println!(
                    "  [{}/{}] {}",
                    entry.chapter.cyan(),
                    entry.seq,
                    entry.effect.describe()
                );
            }
            println!("\n共 {} 条记录", entries.len());
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// JSON serialization helpers
// ══════════════════════════════════════════════════════════════════

fn snapshot_to_json(snap: &Snapshot) -> Value {
    let entities: Vec<Value> = snap.entities.iter().map(|e| {
        let mut obj = json!({
            "type": e.entity_type,
            "id": e.id,
        });
        if let Some(name) = &e.name {
            obj["name"] = json!(name);
        }
        if !e.traits.is_empty() { obj["traits"] = json!(e.traits); }
        if !e.beliefs.is_empty() { obj["beliefs"] = json!(e.beliefs); }
        if !e.desires.is_empty() { obj["desires"] = json!(e.desires); }
        if !e.intentions.is_empty() { obj["intentions"] = json!(e.intentions); }
        if let Some(loc) = &e.location { obj["location"] = json!(loc); }
        if !e.relationships.is_empty() {
            obj["relationships"] = json!(e.relationships.iter().map(|r| json!({
                "target": r.target, "rel": r.rel
            })).collect::<Vec<_>>());
        }
        if !e.inventory.is_empty() { obj["inventory"] = json!(e.inventory); }
        if !e.properties.is_empty() { obj["properties"] = json!(e.properties); }
        if !e.connections.is_empty() { obj["connections"] = json!(e.connections); }
        if !e.tags.is_empty() { obj["tags"] = json!(e.tags); }
        obj
    }).collect();

    let secrets: Vec<Value> = snap.secrets.iter().map(|s| json!({
        "id": s.id,
        "content": s.content,
        "known_by": s.known_by,
        "revealed_to_reader": s.revealed_to_reader,
        "dramatic_function": format!("{:?}", s.classify()),
    })).collect();

    json!({
        "chapter": snap.chapter,
        "entities": entities,
        "secrets": secrets,
    })
}

// ══════════════════════════════════════════════════════════════════
// v3: read phase / beats
// ══════════════════════════════════════════════════════════════════

pub fn read_phase(
    project: &Path,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let state = ledger::ProjectState::load(&everlore);

    let phase_id = match state.active_phase() {
        Some(id) => id.to_string(),
        None => {
            println!("{}", "(没有活跃的 phase)".dimmed());
            return Ok(());
        }
    };

    let phases_dir = everlore.join("phases");
    let phase = ledger::Phase::load(&phases_dir, &phase_id)?;

    match format {
        Format::Json => {
            let j = serde_json::to_value(&phase)?;
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            println!("{}", format!("═══ Phase: {} ═══", phase.id).cyan().bold());
            if let Some(syn) = &phase.synopsis {
                println!("  {}", syn);
            }
            println!();

            // Show constraints summary
            let c = &phase.constraints;
            if !c.ledger.invariants.is_empty() || !c.ledger.exit_state.is_empty() {
                println!("  L1·State:");
                for inv in &c.ledger.invariants {
                    println!("    invariant: {} = {}", inv.query, inv.expected);
                }
                for ex in &c.ledger.exit_state {
                    println!("    exit_state: {} = {}", ex.query, ex.expected);
                }
            }
            if !c.resolver.intents.is_empty() || c.resolver.min_effects.is_some() {
                println!("  L2·Drama:");
                if let Some(n) = c.resolver.min_effects {
                    println!("    min_effects: {n}");
                }
                println!("    intents: {}", c.resolver.intents.len());
            }
            if c.executor.words.is_some() || !c.executor.writing_plan.is_empty() {
                println!("  L3·Writing:");
                if let Some((min, max)) = c.executor.words {
                    println!("    words: {min}-{max}");
                }
                if let Some(pov) = &c.executor.pov {
                    println!("    pov: {pov}");
                }
                if !c.executor.writing_plan.is_empty() {
                    println!("    beats planned: {}", c.executor.writing_plan.len());
                    for bp in &c.executor.writing_plan {
                        println!("      - {} ({}字)", bp.label,
                                 bp.target_words.map_or("?".into(), |w| w.to_string()));
                    }
                }
            }
            if c.evaluator.min_avg_score.is_some() || !c.evaluator.required_tags.is_empty() {
                println!("  L4·Reader:");
                if let Some(s) = c.evaluator.min_avg_score {
                    println!("    min_avg_score: {s}");
                }
                if !c.evaluator.required_tags.is_empty() {
                    println!("    required_tags: {:?}", c.evaluator.required_tags);
                }
            }
        }
    }
    Ok(())
}

pub fn read_beats(
    project: &Path,
    phase_filter: Option<&str>,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let beats_dir = everlore.join("beats");
    let state = ledger::ProjectState::load(&everlore);

    let phase_id = phase_filter
        .map(|s| s.to_string())
        .or_else(|| state.active_phase().map(|s| s.to_string()));

    let Some(phase_id) = phase_id else {
        println!("{}", "(没有活跃的 phase)".dimmed());
        return Ok(());
    };

    let beats = ledger::Beat::load_phase(&beats_dir, &phase_id);

    match format {
        Format::Json => {
            let arr: Vec<Value> = beats.iter().map(|b| json!({
                "seq": b.seq,
                "text": b.text,
                "word_count": b.word_count,
                "effects": b.effects.iter().map(|e| e.describe()).collect::<Vec<_>>(),
                "created_by": b.created_by,
                "revision": b.revision,
            })).collect();
            let response = json!({
                "phase": phase_id,
                "beats": arr,
                "total_words": ledger::Beat::total_words(&beats),
                "total_effects": ledger::Beat::all_effects(&beats).len(),
            });
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Format::Human => {
            if beats.is_empty() {
                println!("{}", format!("(phase '{phase_id}' 尚无 beats)").dimmed());
                return Ok(());
            }
            println!("{}", format!("═══ Beats: {phase_id} ═══").cyan().bold());
            for b in &beats {
                let rev = if b.revision > 0 {
                    format!(" r{}", b.revision).dimmed().to_string()
                } else {
                    String::new()
                };
                println!("\n  #{}{} ({} 字, by {})", b.seq, rev, b.word_count, b.created_by);
                // Show first 100 chars of text
                let preview: String = b.text.chars().take(100).collect();
                println!("  {}", preview.dimmed());
                if b.text.chars().count() > 100 {
                    print!("{}", "...".dimmed());
                }
                if !b.effects.is_empty() {
                    println!("  effects: {:?}", b.effects.iter().map(|e| e.describe()).collect::<Vec<_>>());
                }
            }
            println!("\n  总计: {} beats, {} 字", beats.len(), ledger::Beat::total_words(&beats));
        }
    }
    Ok(())
}
