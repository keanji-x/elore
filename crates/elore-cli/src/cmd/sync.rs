//! `elore sync` — rebuild history.jsonl and state.json from beats/.
//!
//! This is the "safety net" command. After AI directly writes files,
//! `sync` ensures all derived state (history, counters) is consistent.
//!
//! Also registers any phase YAML files that aren't in state.json yet.

use std::path::Path;

use colored::Colorize;

use ledger::effect::beat::Beat;
use ledger::effect::history::History;
use ledger::state::phase::Phase;
use ledger::state::phase_manager::ProjectState;

/// Run `elore sync`: rebuild history + state from filesystem truth.
pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases_dir = everlore.join("phases");
    let beats_dir = everlore.join("beats");
    let entities_dir = everlore.join("entities");

    if !everlore.exists() {
        return Err("项目未初始化 (.everlore/ 不存在)".into());
    }

    println!("{}", "═══ elore sync ═══".cyan().bold());

    // ── Step 1: Register any unregistered phases ──
    let mut state = ProjectState::load(&everlore);
    let mut phases_synced = 0u32;

    if phases_dir.exists() {
        let phase_ids = Phase::list(&phases_dir);
        for phase_id in &phase_ids {
            if !state.phases.contains_key(phase_id) {
                match Phase::load(&phases_dir, phase_id) {
                    Ok(phase) => {
                        state.register_phase(&phase);
                        phases_synced += 1;
                        println!(
                            "  {} 注册 phase: {}",
                            "✓".green(),
                            phase_id.cyan().bold()
                        );
                    }
                    Err(e) => {
                        return Err(format!("无法注册 phase {}: {:?}", phase_id, e).into());
                    }
                }
            }
        }
    }

    // ── Step 2: Rebuild history.jsonl from all beat files ──
    let history_path = everlore.join("history.jsonl");

    // Collect all beats across all phases
    let mut all_entries = Vec::new();
    let mut phase_stats: std::collections::BTreeMap<String, (u32, u32, u32)> =
        std::collections::BTreeMap::new(); // phase_id -> (beats, words, effects)

    if beats_dir.exists() {
        // Get all phase IDs that have beats
        let mut seen_phases: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&beats_dir) {
            for entry in entries.flatten() {
                let stem = entry
                    .path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                // Beat filenames are {phase}_{seq:03}.json or {phase}_{seq:03}_r{rev}.json
                // Extract phase_id as everything before the last _NNN
                if let Some(underscore_pos) = stem.rfind('_') {
                    let candidate = &stem[..underscore_pos];
                    // Handle revision suffix: if candidate also ends in _NNN, go one level up
                    let phase_id = if candidate.ends_with(|c: char| c.is_ascii_digit())
                        && candidate.contains('_')
                    {
                        if let Some(pos) = candidate.rfind('_') {
                            let maybe_phase = &candidate[..pos];
                            // Check if the part after _ is all digits (revision marker)
                            let suffix = &candidate[pos + 1..];
                            if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() <= 3 {
                                maybe_phase.to_string()
                            } else {
                                candidate.to_string()
                            }
                        } else {
                            candidate.to_string()
                        }
                    } else {
                        candidate.to_string()
                    };
                    if !seen_phases.contains(&phase_id) {
                        seen_phases.push(phase_id);
                    }
                }
            }
        }

        // Sort phases by their order in the plan (or alphabetically)
        let plan_order: Vec<String> = state.plan.clone();
        seen_phases.sort_by_key(|p| {
            plan_order
                .iter()
                .position(|x| x == p)
                .unwrap_or(usize::MAX)
        });

        for phase_id in &seen_phases {
            let beats = Beat::load_phase(&beats_dir, phase_id);
            let total_words = Beat::total_words(&beats);
            let total_effects: u32 = beats.iter().map(|b| b.effects.len() as u32).sum();

            phase_stats.insert(
                phase_id.clone(),
                (beats.len() as u32, total_words, total_effects),
            );

            // Collect history entries (in beat order)
            for beat in &beats {
                let entries = beat.as_history_entries();
                all_entries.extend(entries);
            }
        }
    }

    // Write history.jsonl from scratch
    if history_path.exists() {
        std::fs::remove_file(&history_path)?;
    }
    if !all_entries.is_empty() {
        History::append(&everlore, &all_entries)?;
    }
    println!(
        "  {} history.jsonl 重建: {} 条记录",
        "✓".green(),
        all_entries.len()
    );

    // ── Step 3: Update state.json counters ──
    for (phase_id, (beats, words, effects)) in &phase_stats {
        if let Some(entry) = state.phases.get_mut(phase_id) {
            entry.beats = *beats;
            entry.words = *words;
            entry.effects = *effects;
        }
    }

    // ── Step 4: Fill entity defaults (missing optional fields) ──
    let mut entities_fixed = 0u32;
    if entities_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&entities_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    let content = std::fs::read_to_string(&path)?;
                    let mut v = serde_json::from_str::<serde_json::Value>(&content)
                        .map_err(|e| format!("Entity 解析失败 ({}): {}", path.display(), e))?;
                    
                    let mut changed = false;
                    for (key, default) in [
                        ("traits", serde_json::json!([])),
                        ("beliefs", serde_json::json!([])),
                        ("desires", serde_json::json!([])),
                        ("intentions", serde_json::json!([])),
                        ("relationships", serde_json::json!([])),
                        ("inventory", serde_json::json!([])),
                        ("tags", serde_json::json!(["active"])),
                    ] {
                        if v.get(key).is_none() {
                            if let serde_json::Value::Object(ref mut map) = v {
                                map.insert(key.to_string(), default);
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        let pretty = serde_json::to_string_pretty(&v)?;
                        std::fs::write(&path, pretty)?;
                        entities_fixed += 1;
                    }
                }
            }
        }
    }

    state.save(&everlore)?;

    // ── Summary ──
    println!(
        "\n{} sync 完成:",
        "✓".green().bold()
    );
    if phases_synced > 0 {
        println!("  phases 新注册: {}", phases_synced);
    }
    for (phase_id, (beats, words, effects)) in &phase_stats {
        println!(
            "  {} — {} beats, {} 字, {} effects",
            phase_id.cyan(),
            beats,
            words,
            effects
        );
    }
    if entities_fixed > 0 {
        println!("  entities 补充默认字段: {}", entities_fixed);
    }

    Ok(())
}
