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
use ledger::state::content::ContentTree;
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
        // Register phase in state even if it has no beats yet,
        // so that plan/current_phase can be populated.
        let entry = state.phases.entry(phase_id.clone()).or_default();

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

        // Update state counts
        entry.beats = phase_beats;
        entry.words = phase_words;
        entry.effects = phase_effects;

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

    // Ensure plan and current_phase are populated.
    if state.plan.is_empty() && !state.phases.is_empty() {
        state.plan = state.phases.keys().cloned().collect();
    }
    if state.current_phase.is_none() {
        state.current_phase = state.plan.first().cloned();
    }

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

    // ── Step 5: Content tree compilation ─────────────────────
    let content_cards = card::load_content_cards(&cards_dir)?;
    let content_count = content_cards.len();
    if !content_cards.is_empty() {
        let mut tree = ContentTree::load(&everlore);
        let mut orders = std::collections::BTreeMap::new();
        for c in &content_cards {
            orders.insert(c.id.clone(), c.order);
            tree.register(c);
        }
        tree.sort_children(&orders);
        tree.resolve_locks();

        // Validate stale nodes against current snapshot
        let content_map: std::collections::BTreeMap<String, ledger::Content> = content_cards
            .iter()
            .map(|c| (c.id.clone(), c.clone()))
            .collect();

        let stale_ids: Vec<String> = tree
            .nodes
            .iter()
            .filter(|(_, e)| e.stale && e.status == ledger::ContentStatus::Committed)
            .map(|(id, _)| id.clone())
            .collect();

        let mut stale_failures: Vec<(String, Vec<String>)> = Vec::new();
        for nid in &stale_ids {
            let Some(content) = content_map.get(nid) else { continue };
            let mut failures = Vec::new();

            if !content.constraints.exit_state.is_empty() {
                let is_branch = tree.is_branch(nid);
                let snap_id = if is_branch {
                    tree.subtree_leaves(nid).last().cloned().unwrap_or(nid.clone())
                } else {
                    nid.clone()
                };
                if let Ok(snapshot) = ledger::Snapshot::at_content(
                    &snap_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
                ) {
                    let (ok, errs) = ledger::state::constraint::check_assertions(
                        &snapshot, &content.constraints.exit_state,
                    );
                    if !ok {
                        failures.extend(errs);
                    }
                }
            }

            if failures.is_empty() {
                if let Some(entry) = tree.nodes.get_mut(nid.as_str()) {
                    entry.stale = false;
                }
            } else {
                stale_failures.push((nid.clone(), failures));
            }
        }

        tree.save(&everlore)?;

        let content_effects: u32 = content_cards.iter().map(|c| c.effects.len() as u32).sum();
        let content_words: u32 = content_cards.iter().map(|c| c.word_count).sum();

        println!(
            "  {} {} content nodes → content_tree.json ({} effects, {} 字)",
            "✓".green(),
            content_count,
            content_effects,
            content_words,
        );

        // Print tree structure
        if let Some(ref root) = tree.root {
            print_content_tree(&tree, root, 0);
        }

        // Report stale validation failures
        if !stale_failures.is_empty() {
            println!();
            println!("{}", "  ⚠ stale 节点验证失败:".yellow().bold());
            for (nid, failures) in &stale_failures {
                println!("    {} — {}", nid.yellow(), failures.join(", "));
            }
        }
    }

    // ── Summary ────────────────────────────────────────────────
    println!();
    println!(
        "{} build 完成: {} entities, {} secrets, {} beats ({} 字), {} content nodes",
        "✓".green().bold(),
        entities.len(),
        secrets.len(),
        total_beats,
        total_words,
        content_count,
    );

    Ok(())
}

fn print_content_tree(tree: &ContentTree, node: &str, depth: usize) {
    let indent = "  ".repeat(depth + 2);
    let entry = tree.nodes.get(node);
    let status = entry
        .map(|e| match e.status {
            ledger::ContentStatus::Locked => "🔒",
            ledger::ContentStatus::Active => "🔵",
            ledger::ContentStatus::Committed if e.stale => "⚠️",
            ledger::ContentStatus::Committed => "✅",
        })
        .unwrap_or("?");
    let prefix = if depth == 0 { "" } else { "├── " };
    let version = entry.map(|e| e.version).unwrap_or(1);
    let ver_str = if version > 1 {
        format!(" v{version}")
    } else {
        String::new()
    };
    let stale_str = if entry.is_some_and(|e| e.stale) {
        " (stale)".yellow().to_string()
    } else {
        String::new()
    };
    println!("{indent}{prefix}{status} {}{ver_str}{stale_str}", node.bold());
    for child in tree.children_of(node) {
        print_content_tree(tree, child, depth + 1);
    }
}
