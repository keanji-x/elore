//! `elore build` — compile cards/ into .everlore/ artifacts.
//!
//! Cards are the source of truth. This command:
//! 1. Parses all entity/secret/beat cards
//! 2. Writes entity JSON cache to .everlore/entities/
//! 3. Writes secrets cache to .everlore/secrets.yaml
//! 4. Rebuilds history.jsonl from beat card effects
//! 5. Updates state.json progress counters

use std::path::Path;

use colored::Colorize;

use std::collections::HashMap;
use std::path::PathBuf;

use ledger::Beat;
use ledger::card;
use ledger::effect::history::History;
use ledger::input::card_writer;
use ledger::input::secret;
use ledger::state::phase::Phase;
use ledger::state::phase_manager::ProjectState;

pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");

    if !cards_dir.exists() {
        return Err(format!(
            "cards/ 目录不存在 — 请先运行 `elore init` 或创建 cards/ 目录"
        )
        .into());
    }

    // Ensure output dirs exist
    let entities_dir = everlore.join("entities");
    let beats_dir = everlore.join("beats");
    std::fs::create_dir_all(&entities_dir)?;
    std::fs::create_dir_all(&beats_dir)?;

    println!("{}", "═══ elore build ═══".cyan().bold());

    // ── Step 1: Load entity cards ──────────────────────────────
    let entity_pairs = card::load_entity_cards(&cards_dir)?;
    let entities: Vec<_> = entity_pairs.iter().map(|(e, _)| e.clone()).collect();

    // Write entity JSON cache
    // Clear old cache first
    if entities_dir.exists() {
        for entry in std::fs::read_dir(&entities_dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                std::fs::remove_file(path)?;
            }
        }
    }

    for e in &entities {
        let path = entities_dir.join(format!("{}.json", e.id()));
        let content = serde_json::to_string_pretty(e)?;
        std::fs::write(path, content)?;
    }
    println!(
        "  {} {} entities → .everlore/entities/",
        "✓".green(),
        entities.len()
    );

    // ── Step 2: Load secret cards ──────────────────────────────
    let secrets = card::load_secret_cards(&cards_dir)?;

    // Write secrets cache
    let secrets_file = secret::SecretsFileOut {
        secrets: secrets.clone(),
    };
    let secrets_yaml = serde_yaml::to_string(&secrets_file)?;
    std::fs::write(everlore.join("secrets.yaml"), secrets_yaml)?;
    println!(
        "  {} {} secrets → .everlore/secrets.yaml",
        "✓".green(),
        secrets.len()
    );

    // ── Step 3: Load phases and beat cards ──────────────────────
    let phases_dir = everlore.join("phases");
    let phase_ids = Phase::list(&phases_dir);

    // Also check cards/phases/ for phase directories
    let cards_phases_dir = cards_dir.join("phases");
    let mut beat_phase_ids: Vec<String> = Vec::new();
    if cards_phases_dir.exists() {
        for entry in std::fs::read_dir(&cards_phases_dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    beat_phase_ids.push(name.to_string());
                }
            }
        }
    }
    beat_phase_ids.sort();

    // Use all known phase IDs (from phase definitions + beat card directories)
    let all_phase_ids: Vec<String> = {
        let mut ids = phase_ids;
        for id in &beat_phase_ids {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        ids
    };

    // Rebuild history.jsonl from beat cards
    let mut all_history_entries = Vec::new();
    let mut total_beats = 0u32;
    let mut total_words = 0u32;

    let mut state = ProjectState::load(&everlore);

    for phase_id in &all_phase_ids {
        let beats = card::load_beat_cards(&cards_dir, phase_id)?;
        if beats.is_empty() {
            continue;
        }

        let phase_beats = beats.len() as u32;
        let phase_words: u32 = beats.iter().map(|b| b.word_count).sum();
        let phase_effects: u32 = beats.iter().map(|b| b.effects.len() as u32).sum();

        // Save beat JSON cache
        for beat in &beats {
            beat.save(&beats_dir)?;
        }

        // Collect history entries
        let entries = Beat::to_history_entries(&beats);
        all_history_entries.extend(entries);

        // Update state
        if let Some(entry) = state.phases.get_mut(phase_id) {
            entry.beats = phase_beats;
            entry.words = phase_words;
            entry.effects = phase_effects;
        }

        total_beats += phase_beats;
        total_words += phase_words;

        println!(
            "  {} phase {} — {} beats, {} 字, {} effects",
            "✓".green(),
            phase_id.cyan(),
            phase_beats,
            phase_words,
            phase_effects
        );
    }

    // Write history.jsonl
    let history_path = everlore.join("history.jsonl");
    let mut history_content = String::new();
    for entry in &all_history_entries {
        let line = serde_json::to_string(entry)?;
        history_content.push_str(&line);
        history_content.push('\n');
    }
    std::fs::write(&history_path, history_content)?;
    println!(
        "  {} {} history entries → history.jsonl",
        "✓".green(),
        all_history_entries.len()
    );

    // Save state
    state.save(&everlore)?;

    // ── Step 4: Reverse sync — apply effects and update cards ──
    if !all_history_entries.is_empty() {
        // Build source map: entity_id → card file path
        let source_map: HashMap<String, PathBuf> = entity_pairs
            .iter()
            .map(|(e, p)| (e.id().to_string(), p.clone()))
            .collect();

        // Replay effects on entities
        let mut synced_entities = entities.clone();
        let history = History { entries: all_history_entries.clone() };
        History::replay_entities(&mut synced_entities, &history, None);

        // Write updated state back to card files
        let mut updated_count = 0;
        for entity in &synced_entities {
            if let Some(card_path) = source_map.get(entity.id()) {
                card_writer::update_entity_card(card_path, entity)?;
                updated_count += 1;
            }
        }

        // Also replay secrets
        let mut synced_secrets = secrets.clone();
        History::replay_secrets(&mut synced_secrets, &history, None);

        // Build secret source map
        let secret_dir = cards_dir.join("secrets");
        if secret_dir.exists() {
            for secret in &synced_secrets {
                let secret_path = secret_dir.join(format!("{}.md", secret.id));
                if secret_path.exists() {
                    card_writer::update_secret_card(&secret_path, secret)?;
                }
            }
        }

        // Also update entity JSON cache with post-effect state
        for e in &synced_entities {
            let path = entities_dir.join(format!("{}.json", e.id()));
            let content = serde_json::to_string_pretty(e)?;
            std::fs::write(path, content)?;
        }

        if updated_count > 0 {
            println!(
                "  {} {} cards 已更新 (effects 反向同步)",
                "✓".green(),
                updated_count
            );
        }
    }

    // ── Summary ────────────────────────────────────────────────
    println!();
    println!(
        "{} build 完成: {} entities, {} secrets, {} beats ({} 字)",
        "✓".green().bold(),
        entities.len(),
        secrets.len(),
        total_beats,
        total_words,
    );

    Ok(())
}
