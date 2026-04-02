//! `elore content` — tree-based narrative management commands.
//!
//! Commands: list (tree view), show (node details), activate, commit, snapshot.

use std::collections::BTreeMap;
use std::path::Path;

use colored::Colorize;
use serde_json::json;

use ledger::card;
use ledger::state::content::{Content, ContentStatus, ContentTree, check_slot_coverage, effective_style, extract_refs, extract_context, strip_refs};
use ledger::state::snapshot::Snapshot;

use crate::cmd::Format;

// ══════════════════════════════════════════════════════════════════
// list — print the content tree
// ══════════════════════════════════════════════════════════════════

pub fn list(project: &Path, format: Format) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    // Load content cards for display data (words, effects)
    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    match format {
        Format::Json => {
            let j = json!({
                "root": tree.root,
                "active": tree.active,
                "nodes": tree.nodes,
                "children": tree.children,
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            if tree.root.is_none() {
                println!(
                    "{}",
                    "(尚无 content 树，请在 cards/content/ 下创建 content 卡片后运行 `elore build`)"
                        .dimmed()
                );
                return Ok(());
            }
            println!("{}", "═══ Content Tree ═══".cyan().bold());
            if let Some(ref active) = tree.active {
                println!("  active: {}", active.cyan().bold());
            }
            println!();
            print_tree(&tree, &content_map, tree.root.as_deref().unwrap(), 0);
        }
    }
    Ok(())
}

fn print_tree(
    tree: &ContentTree,
    content_map: &BTreeMap<String, Content>,
    node: &str,
    depth: usize,
) {
    let indent = "  ".repeat(depth + 1);
    let entry = tree.nodes.get(node);
    let status_icon = entry
        .map(|e| match e.status {
            ContentStatus::Locked => "🔒",
            ContentStatus::Active => "🔵",
            ContentStatus::Committed => "✅",
        })
        .unwrap_or("?");

    let is_active = tree.active.as_deref() == Some(node);
    let is_branch = tree.is_branch(node);
    let content = content_map.get(node);
    let words = content.map(|c| c.word_count).unwrap_or(0);
    let effects = content.map(|c| c.effects.len()).unwrap_or(0);

    let prefix = if depth == 0 { String::new() } else { "├── ".to_string() };
    let cursor = if is_active { " ◀".cyan().bold().to_string() } else { String::new() };

    if is_branch {
        println!(
            "{indent}{prefix}{status_icon} {} {} ({} effects){cursor}",
            node.bold(),
            "[branch]".dimmed(),
            effects
        );
    } else if words > 0 || effects > 0 {
        println!(
            "{indent}{prefix}{status_icon} {} ({} 字, {} effects){cursor}",
            node.bold(),
            words,
            effects
        );
    } else {
        println!("{indent}{prefix}{status_icon} {}{cursor}", node.bold());
    }

    for child in tree.children_of(node) {
        print_tree(tree, content_map, child, depth + 1);
    }
}

// ══════════════════════════════════════════════════════════════════
// show — display details of a content node
// ══════════════════════════════════════════════════════════════════

pub fn show(
    project: &Path,
    content_id: &str,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    let contents = card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> = contents
        .iter()
        .map(|c| (c.id.clone(), c.clone()))
        .collect();
    let content = contents
        .iter()
        .find(|c| c.id == content_id)
        .ok_or_else(|| format!("Content '{content_id}' 不存在"))?;

    let entry = tree.nodes.get(content_id);
    let ancestors = tree.ancestors(content_id);
    let children: Vec<String> = tree.children_of(content_id).to_vec();
    let is_branch = tree.is_branch(content_id);
    let replay_path = tree.replay_path(content_id);

    match format {
        Format::Json => {
            let j = json!({
                "id": content.id,
                "parent": content.parent,
                "order": content.order,
                "title": content.title,
                "synopsis": content.synopsis,
                "word_count": content.word_count,
                "effects_count": content.effects.len(),
                "is_branch": is_branch,
                "status": entry.map(|e| &e.status),
                "ancestors": ancestors,
                "children": children,
                "replay_path": replay_path,
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            let status = entry
                .map(|e| match e.status {
                    ContentStatus::Locked => "🔒 locked".to_string(),
                    ContentStatus::Active => "🔵 active".to_string(),
                    ContentStatus::Committed => "✅ committed".to_string(),
                })
                .unwrap_or_else(|| "? unknown".to_string());

            let node_type = if is_branch { "branch" } else { "leaf" };

            println!("{}", "═══ Content Node ═══".cyan().bold());
            println!("  id:       {}", content_id.bold());
            println!("  type:     {node_type}");
            println!("  status:   {status}");
            if let Some(ref title) = content.title {
                println!("  title:    {title}");
            }
            if let Some(ref parent) = content.parent {
                println!("  parent:   {parent}");
            }
            println!("  order:    {}", content.order);

            if is_branch {
                println!("  effects:  {} (budget for children)", content.effects.len());
            } else {
                println!(
                    "  words:    {}, effects: {}",
                    content.word_count,
                    content.effects.len()
                );
            }

            // main_role (with inheritance)
            let effective_role = ledger::effective_main_role(content_id, &tree, &content_map);
            if let Some(ref role) = effective_role {
                let inherited = content.main_role.is_none();
                if inherited {
                    println!("  main_role: {} {}", role.yellow(), "(inherited)".dimmed());
                } else {
                    println!("  main_role: {}", role.yellow());
                }
            }

            if !ancestors.is_empty() {
                println!("  path:     {}", ancestors.join(" → ").dimmed());
            }
            if !children.is_empty() {
                println!("  children: {}", children.join(", "));
            }

            if !content.constraints.exit_state.is_empty() {
                println!(
                    "  exit_state: {} assertions",
                    content.constraints.exit_state.len()
                );
            }
            if let Some((min, max)) = content.constraints.words {
                println!("  word_range: {min}-{max}");
            }

            let style = effective_style(content_id, &tree, &content_map);
            if !style.is_empty() {
                println!("  style:    {}", style.join(", ").yellow());
            }

            if !is_branch && !content.text.is_empty() {
                println!();
                let preview = if content.text.chars().count() > 200 {
                    let s: String = content.text.chars().take(200).collect();
                    format!("{s}…")
                } else {
                    content.text.clone()
                };
                println!("  {}", preview.dimmed());
            }

            // Perspective drafts
            let drafts = ledger::load_pov_drafts(&cards_dir, content_id);
            if !drafts.is_empty() {
                println!();
                println!("  {}", "视角草稿:".cyan());
                for d in &drafts {
                    let chars = d.text.chars().count();
                    println!("    [{}] {} ({} 字)", d.category.dir_name(), d.name.bold(), chars);
                }
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// activate — start working on a content node
// ══════════════════════════════════════════════════════════════════

pub fn activate(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let mut tree = ContentTree::load(&everlore);

    tree.activate(content_id)?;
    tree.save(&everlore)?;

    println!(
        "{} activate → {}",
        "✓".green().bold(),
        content_id.cyan().bold()
    );
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// edit — open a node for editing (all side effects live here)
// ══════════════════════════════════════════════════════════════════

pub fn edit(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let mut tree = ContentTree::load(&everlore);

    let stale_nodes = tree.edit(content_id)?;
    tree.save(&everlore)?;

    // Create perspective directories
    let node_dir = ledger::card::content_node_dir(&cards_dir, content_id);
    for cat in ledger::PovCategory::ALL {
        let _ = std::fs::create_dir_all(node_dir.join(cat.dir_name()));
    }

    // Initialize progress (sync existing drafts)
    let mut progress = ledger::Progress::load(&cards_dir, content_id);
    progress.sync_drafts(&cards_dir, content_id);
    let _ = progress.save(&cards_dir, content_id);

    let version = tree
        .nodes
        .get(content_id)
        .map(|e| e.version)
        .unwrap_or(1);

    println!(
        "{} edit → {} (v{})",
        "✓".green().bold(),
        content_id.cyan().bold(),
        version
    );
    println!("  pov/ timeline/ outsider/ 已就绪");

    if !stale_nodes.is_empty() {
        println!(
            "  {} {} downstream nodes marked stale",
            "⚠".yellow(),
            stale_nodes.len()
        );
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// commit — finalize a content node (L1–L4 checks)
// ══════════════════════════════════════════════════════════════════

pub fn commit(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let entities_dir = everlore.join("entities");

    let mut tree = ContentTree::load(&everlore);

    let contents = card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> = contents
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    // Recursively commit subtree (DFS, children first, then self)
    commit_subtree(
        content_id,
        &mut tree,
        &content_map,
        &entities_dir,
        &cards_dir,
        &everlore,
    )?;

    // Re-validate stale downstream nodes
    let stale_nodes: Vec<String> = tree
        .downstream_of(content_id)
        .into_iter()
        .filter(|nid| {
            tree.nodes
                .get(nid.as_str())
                .is_some_and(|e| e.stale && e.status == ContentStatus::Committed)
        })
        .collect();

    if !stale_nodes.is_empty() {
        println!();
        println!(
            "{}",
            "── 下游验证 ──".dimmed()
        );
        let mut all_ok = true;
        for nid in &stale_nodes {
            let Some(content) = content_map.get(nid.as_str()) else {
                continue;
            };
            let is_branch = tree.is_branch(nid);
            let mut failures = Vec::new();

            // Re-check exit_state constraints against current snapshot
            if !content.constraints.exit_state.is_empty() {
                let snap_id = if is_branch {
                    tree.subtree_leaves(nid).last().cloned().unwrap_or(nid.clone())
                } else {
                    nid.clone()
                };
                if let Ok(snapshot) = Snapshot::at_content(
                    &snap_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
                ) {
                    let (ok, errs) = ledger::state::constraint::check_assertions(
                        &snapshot, &content.constraints.exit_state,
                    );
                    if !ok {
                        for f in &errs {
                            failures.push(format!("exit_state: {f}"));
                        }
                    }
                }
            }

            // Re-check invariants
            if !is_branch && !content.constraints.invariants.is_empty() {
                if let Ok(snapshot) = Snapshot::at_content(
                    nid, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
                ) {
                    let (ok, errs) = ledger::state::constraint::check_assertions(
                        &snapshot, &content.constraints.invariants,
                    );
                    if !ok {
                        for f in &errs {
                            failures.push(format!("invariant: {f}"));
                        }
                    }
                }
            }

            if failures.is_empty() {
                // Passed — clear stale
                if let Some(entry) = tree.nodes.get_mut(nid.as_str()) {
                    entry.stale = false;
                }
                println!("  {} {} ✓", "✓".green(), nid);
            } else {
                all_ok = false;
                println!("  {} {} — 约束失败:", "⚠".yellow(), nid.yellow());
                for f in &failures {
                    println!("    - {f}");
                }
            }
        }
        if all_ok {
            println!(
                "  {} 所有下游节点验证通过，stale 已清除",
                "✓".green().bold()
            );
        }
    }

    tree.save(&everlore)?;
    Ok(())
}

/// Recursively commit a subtree: children first (DFS), then self.
/// Skips already-committed nodes.
fn commit_subtree(
    content_id: &str,
    tree: &mut ContentTree,
    content_map: &BTreeMap<String, Content>,
    entities_dir: &Path,
    cards_dir: &Path,
    everlore: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = tree
        .nodes
        .get(content_id)
        .ok_or_else(|| format!("Content '{content_id}' 未注册"))?;

    // Skip already committed
    if entry.status == ContentStatus::Committed {
        return Ok(());
    }

    // Skip locked (can't commit yet)
    if entry.status == ContentStatus::Locked {
        return Err(format!(
            "Content '{content_id}' 是 locked 状态，不能 commit"
        ).into());
    }

    let is_branch = tree.is_branch(content_id);

    // If branch: recursively commit children first
    if is_branch {
        let children: Vec<String> = tree.children_of(content_id).to_vec();
        for child_id in &children {
            commit_subtree(child_id, tree, content_map, entities_dir, cards_dir, everlore)?;
        }
    }

    // Now commit self
    let content = content_map
        .get(content_id)
        .ok_or_else(|| format!("Content card '{content_id}' 不存在"))?;

    let mut blockers = Vec::new();

    if is_branch {
        // ── Branch commit ──
        if !tree.all_children_committed(content_id) {
            blockers.push("并非所有 children 都已 committed".to_string());
        }

        let children_effects: Vec<Vec<ledger::Op>> = tree
            .children_of(content_id)
            .iter()
            .filter_map(|child_id| content_map.get(child_id.as_str()))
            .map(|c| c.effects.clone())
            .collect();

        let (_, uncovered) = check_slot_coverage(&content.effects, &children_effects);
        if !uncovered.is_empty() {
            for u in &uncovered {
                blockers.push(format!("未被 children 覆盖: {u}"));
            }
        }

        if !content.constraints.exit_state.is_empty() {
            let subtree_leaves = tree.subtree_leaves(content_id);
            if let Some(last_leaf) = subtree_leaves.last() {
                let snapshot = Snapshot::at_content(
                    last_leaf, tree, content_map, entities_dir, cards_dir, everlore,
                )?;
                let (ok, failures) = ledger::state::constraint::check_assertions(
                    &snapshot, &content.constraints.exit_state,
                );
                if !ok {
                    for f in &failures {
                        blockers.push(format!("L1 exit_state (subtree end): {f}"));
                    }
                }
            }
        }
    } else {
        // ── Leaf commit ──
        let snapshot = Snapshot::at_content(
            content_id, tree, content_map, entities_dir, cards_dir, everlore,
        )?;

        if !content.constraints.exit_state.is_empty() {
            let (ok, failures) = ledger::state::constraint::check_assertions(
                &snapshot, &content.constraints.exit_state,
            );
            if !ok {
                for f in &failures {
                    blockers.push(format!("L1 exit_state: {f}"));
                }
            }
        }

        if !content.constraints.invariants.is_empty() {
            let (ok, failures) = ledger::state::constraint::check_assertions(
                &snapshot, &content.constraints.invariants,
            );
            if !ok {
                for f in &failures {
                    blockers.push(format!("L1 invariant: {f}"));
                }
            }
        }

        if let Some(min) = content.constraints.min_effects {
            let count = content.effects.len() as u32;
            if count < min {
                blockers.push(format!("L2 effects: {count}/{min}"));
            }
        }

        if let Some((min, max)) = content.constraints.words {
            if content.word_count < min || content.word_count > max {
                blockers.push(format!("L3 words: {}/{min}-{max}", content.word_count));
            }
        }

        // Progress checks
        if let Some(min) = content.constraints.min_povs {
            let mut progress = ledger::Progress::load(cards_dir, content_id);
            progress.sync_drafts(cards_dir, content_id);

            let total = progress.drafts.total() as u32;
            if total < min {
                blockers.push(format!("视角草稿: {total}/{min}"));
            }
            if !progress.drafts.has_all_categories() {
                let mut missing = Vec::new();
                if progress.drafts.pov.is_empty() { missing.push("pov/"); }
                if progress.drafts.timeline.is_empty() { missing.push("timeline/"); }
                if progress.drafts.outsider.is_empty() { missing.push("outsider/"); }
                blockers.push(format!("缺少视角类别: {}", missing.join(", ")));
            }
            if progress.context_checked.is_empty() {
                blockers.push("未执行 context（至少查看一个角色）".to_string());
            }
            if !progress.suggest_ran {
                blockers.push("未执行 suggest".to_string());
            }

            let _ = progress.save(cards_dir, content_id);
        }
    }

    if !blockers.is_empty() {
        println!(
            "{} {} 不能 commit — 约束未满足:",
            "✗".red().bold(),
            content_id.cyan().bold()
        );
        for b in &blockers {
            println!("  - {b}");
        }
        return Err("content 约束未满足".into());
    }

    tree.commit(content_id)?;

    let node_type = if is_branch { "branch" } else { "leaf" };
    println!(
        "{} {} ({}) → committed ✅",
        "✓".green().bold(),
        content_id.cyan().bold(),
        node_type
    );

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// diff — show before/after world state changes
// ══════════════════════════════════════════════════════════════════

pub fn diff(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let entities_dir = everlore.join("entities");

    let tree = ContentTree::load(&everlore);
    let contents = card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> = contents
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let before = Snapshot::before_content(
        content_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
    )?;
    let after = Snapshot::at_content(
        content_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
    )?;

    let diff = ledger::effect::diff::SnapshotDiff::compute(&before, &after);

    let content = content_map.get(content_id);
    let title = content
        .and_then(|c| c.title.as_deref())
        .unwrap_or(content_id);

    println!("{} {}", "═══ Diff:".cyan().bold(), title.bold());

    if !diff.added_entities.is_empty() {
        for id in &diff.added_entities {
            println!("  {} entity {}", "+".green().bold(), id.bold());
        }
    }
    if !diff.removed_entities.is_empty() {
        for id in &diff.removed_entities {
            println!("  {} entity {}", "-".red().bold(), id.bold());
        }
    }

    if diff.entity_diffs.is_empty() && diff.added_entities.is_empty() && diff.removed_entities.is_empty() {
        println!("  {}", "(无变化)".dimmed());
    }

    for ed in &diff.entity_diffs {
        let has_changes = ed.location_change.is_some()
            || !ed.added_traits.is_empty()
            || !ed.removed_traits.is_empty()
            || !ed.added_beliefs.is_empty()
            || !ed.removed_beliefs.is_empty()
            || !ed.added_relationships.is_empty()
            || !ed.removed_relationships.is_empty()
            || !ed.added_inventory.is_empty()
            || !ed.removed_inventory.is_empty();

        if !has_changes {
            continue;
        }

        println!("  {} ({})", ed.id.bold(), ed.entity_type.dimmed());

        if let Some((from, to)) = &ed.location_change {
            let from = from.as_deref().unwrap_or("?");
            let to = to.as_deref().unwrap_or("?");
            println!("    location: {} → {}", from.red(), to.green());
        }
        for t in &ed.added_traits {
            println!("    {} trait {}", "+".green(), t);
        }
        for t in &ed.removed_traits {
            println!("    {} trait {}", "-".red(), t);
        }
        for b in &ed.added_beliefs {
            println!("    {} belief {}", "+".green(), b);
        }
        for b in &ed.removed_beliefs {
            println!("    {} belief {}", "-".red(), b);
        }
        for (target, role) in &ed.added_relationships {
            println!("    {} rel {}({})", "+".green(), target, role);
        }
        for (target, role) in &ed.removed_relationships {
            println!("    {} rel {}({})", "-".red(), target, role);
        }
        for i in &ed.added_inventory {
            println!("    {} item {}", "+".green(), i);
        }
        for i in &ed.removed_inventory {
            println!("    {} item {}", "-".red(), i);
        }
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// snapshot — show world state at a content node
// ══════════════════════════════════════════════════════════════════

pub fn snapshot(
    project: &Path,
    content_id: &str,
    format: Format,
    before: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let entities_dir = everlore.join("entities");

    let tree = ContentTree::load(&everlore);
    let contents = card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> = contents
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let snap = if before {
        Snapshot::before_content(
            content_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
        )?
    } else {
        Snapshot::at_content(
            content_id, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
        )?
    };

    match format {
        Format::Json => {
            let entities: Vec<serde_json::Value> = snap
                .entities
                .iter()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .collect();
            let secrets: Vec<serde_json::Value> = snap
                .secrets
                .iter()
                .map(|s| serde_json::to_value(s).unwrap_or_default())
                .collect();
            let j = json!({
                "content_id": content_id,
                "replay_path": tree.replay_path(content_id),
                "entities": entities,
                "secrets": secrets,
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            println!("{}", "═══ Content Snapshot ═══".cyan().bold());
            println!("  at: {}", content_id.bold());
            println!(
                "  replay_path: {}",
                tree.replay_path(content_id).join(" → ").dimmed()
            );
            println!();
            println!(
                "  {} characters, {} locations, {} factions, {} secrets",
                snap.characters().len(),
                snap.locations().len(),
                snap.factions().len(),
                snap.secrets.len(),
            );
            println!();
            for entity in &snap.entities {
                let loc = entity
                    .as_character()
                    .and_then(|c| c.location.as_deref())
                    .unwrap_or("-");
                println!(
                    "  {} {} ({}@{})",
                    match entity.entity_type() {
                        "character" => "👤",
                        "location" => "📍",
                        "faction" => "⚔️",
                        _ => "•",
                    },
                    entity.id().bold(),
                    entity.entity_type(),
                    loc
                );
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read level — synopsis at a given depth
// ══════════════════════════════════════════════════════════════════

pub fn read_level(project: &Path, depth: usize) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let nodes = tree.nodes_at_depth(depth);

    if nodes.is_empty() {
        println!("{}", format!("(depth {depth} 没有节点)").dimmed());
        return Ok(());
    }

    let root_title = tree
        .root
        .as_deref()
        .and_then(|r| content_map.get(r))
        .and_then(|c| c.title.as_deref())
        .unwrap_or("untitled");
    println!(
        "{} depth={depth}",
        format!("═══ {root_title} ═══").cyan().bold()
    );
    println!();

    for (i, id) in nodes.iter().enumerate() {
        let content = content_map.get(id.as_str());
        let title = content.and_then(|c| c.title.as_deref()).unwrap_or(id);
        let synopsis = content.and_then(|c| c.synopsis.as_deref()).unwrap_or("");
        let is_branch = tree.is_branch(id);
        let leaf_count = if is_branch {
            format!(" ({} leaves)", tree.subtree_leaves(id).len())
        } else {
            let words = content.map(|c| c.word_count).unwrap_or(0);
            format!(" ({words} 字)")
        };

        println!("  {}. {}{}", (i + 1), title.bold(), leaf_count.dimmed());
        if !synopsis.is_empty() {
            println!("     {}", synopsis.dimmed());
        }
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read leaf — concatenated leaf text up to a node
// ══════════════════════════════════════════════════════════════════

pub fn read_leaf(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    if !tree.nodes.contains_key(content_id) {
        return Err(format!("Content '{content_id}' 不存在").into());
    }

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    // Get all leaves up to and including this node (or its subtree leaves if branch)
    let leaves = if tree.is_leaf(content_id) {
        tree.leaf_replay_path(content_id)
    } else {
        // For branch: all leaves up to the end of this subtree
        let subtree_leaves = tree.subtree_leaves(content_id);
        let last = subtree_leaves.last().cloned().unwrap_or_default();
        tree.leaf_replay_path(&last)
    };

    let root_title = tree
        .root
        .as_deref()
        .and_then(|r| content_map.get(r))
        .and_then(|c| c.title.as_deref())
        .unwrap_or("untitled");

    let target_title = content_map
        .get(content_id)
        .and_then(|c| c.title.as_deref())
        .unwrap_or(content_id);

    println!(
        "{} → {}",
        format!("═══ {root_title} ═══").cyan().bold(),
        target_title.bold()
    );
    println!();

    let mut total_words: u32 = 0;
    for leaf_id in &leaves {
        if let Some(content) = content_map.get(leaf_id.as_str()) {
            if !content.text.is_empty() {
                let title = content.title.as_deref().unwrap_or(leaf_id);
                println!("{}", format!("── {title} ──").dimmed());
                println!();
                println!("{}", content.text);
                println!();
                total_words += content.word_count;
            }
        }
    }

    println!(
        "{}",
        format!("── {} leaves, {total_words} 字 ──", leaves.len()).dimmed()
    );

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read path — context chain from root to a node
// ══════════════════════════════════════════════════════════════════

pub fn read_path(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    if !tree.nodes.contains_key(content_id) {
        return Err(format!("Content '{content_id}' 不存在").into());
    }

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let ancestors = tree.ancestors(content_id);

    println!("{}", "═══ Path ═══".cyan().bold());
    println!();

    for (i, id) in ancestors.iter().enumerate() {
        let content = content_map.get(id.as_str());
        let title = content.and_then(|c| c.title.as_deref()).unwrap_or(id);
        let synopsis = content.and_then(|c| c.synopsis.as_deref()).unwrap_or("");
        let effects = content.map(|c| &c.effects).cloned().unwrap_or_default();
        let is_branch = tree.is_branch(id);

        let indent = "  ".repeat(i);
        let connector = if i == 0 { "" } else { "└─ " };
        let node_type = if is_branch { " [branch]" } else { "" };

        println!("{indent}{connector}{}{node_type}", title.bold());

        if !synopsis.is_empty() {
            println!("{indent}   {}", synopsis.dimmed());
        }

        if !effects.is_empty() {
            let effect_strs: Vec<String> = effects.iter().map(|op| op.describe()).collect();
            println!(
                "{indent}   effects: {}",
                effect_strs.join(", ").yellow()
            );
        }

        let node_style = content.map(|c| &c.style).cloned().unwrap_or_default();
        let node_override = content.is_some_and(|c| c.style_override);
        if !node_style.is_empty() {
            let prefix = if node_override { "style (override): " } else { "style: +" };
            println!("{indent}   {}{}", prefix.dimmed(), node_style.join(", ").yellow());
        }

        if i < ancestors.len() - 1 {
            println!();
        }
    }

    // Show final effective style
    let eff_style = effective_style(content_id, &tree, &content_map);
    if !eff_style.is_empty() {
        println!();
        println!(
            "  {} {}",
            "effective style:".cyan(),
            eff_style.join(", ").yellow()
        );
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read parent — parent synopsis + sibling overview
// ══════════════════════════════════════════════════════════════════

pub fn read_parent(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    if !tree.nodes.contains_key(content_id) {
        return Err(format!("Content '{content_id}' 不存在").into());
    }

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let parent_id = match tree.parents.get(content_id) {
        Some(p) => p.clone(),
        None => {
            println!("{}", "(根节点，没有父节点)".dimmed());
            return Ok(());
        }
    };

    let parent = content_map.get(parent_id.as_str());
    let parent_title = parent.and_then(|c| c.title.as_deref()).unwrap_or(&parent_id);
    let parent_synopsis = parent.and_then(|c| c.synopsis.as_deref()).unwrap_or("");

    println!(
        "{} {}",
        "═══".cyan().bold(),
        parent_title.bold()
    );
    if !parent_synopsis.is_empty() {
        println!("  {}", parent_synopsis.dimmed());
    }

    let effects = parent.map(|c| &c.effects).cloned().unwrap_or_default();
    if !effects.is_empty() {
        let strs: Vec<String> = effects.iter().map(|op| op.describe()).collect();
        println!("  effects: {}", strs.join(", ").yellow());
    }
    println!();

    // List all siblings
    let siblings = tree.children_of(&parent_id);
    let mut next_up: Option<String> = None;
    let mut found_current = false;

    for sib_id in siblings {
        let entry = tree.nodes.get(sib_id.as_str());
        let sib_content = content_map.get(sib_id.as_str());
        let title = sib_content.and_then(|c| c.title.as_deref()).unwrap_or(sib_id);
        let synopsis = sib_content.and_then(|c| c.synopsis.as_deref()).unwrap_or("");
        let words = sib_content.map(|c| c.word_count).unwrap_or(0);
        let is_branch = tree.is_branch(sib_id);

        let status_icon = entry
            .map(|e| match e.status {
                ContentStatus::Locked => "🔒",
                ContentStatus::Active => "🔵",
                ContentStatus::Committed => "✅",
            })
            .unwrap_or("?");

        let is_current = sib_id == content_id;
        let marker = if is_current { " ◀".cyan().bold().to_string() } else { String::new() };

        let info = if is_branch {
            format!("[branch, {} leaves]", tree.subtree_leaves(sib_id).len())
        } else if words > 0 {
            format!("({words} 字)")
        } else {
            String::new()
        };

        println!("  {status_icon} {} {}{marker}", title.bold(), info.dimmed());
        if !synopsis.is_empty() {
            println!("     {}", synopsis.dimmed());
        }

        if is_current {
            found_current = true;
        } else if found_current && next_up.is_none() {
            next_up = Some(sib_id.clone());
        }
    }

    // Show what's next
    println!();
    if let Some(next) = next_up {
        let next_title = content_map.get(next.as_str())
            .and_then(|c| c.title.as_deref())
            .unwrap_or(&next);
        let next_status = tree.nodes.get(next.as_str())
            .map(|e| match e.status {
                ContentStatus::Locked => "🔒",
                ContentStatus::Active => "🔵",
                ContentStatus::Committed => "✅",
            })
            .unwrap_or("?");
        println!("  → 下一个: {next_status} {}", next_title.cyan());
    } else {
        // Check parent's next sibling
        if let Some(grandparent) = tree.parents.get(&parent_id) {
            let parent_siblings = tree.children_of(grandparent);
            let mut found_parent = false;
            for sib in parent_siblings {
                if sib == &parent_id {
                    found_parent = true;
                } else if found_parent {
                    let sib_title = content_map.get(sib.as_str())
                        .and_then(|c| c.title.as_deref())
                        .unwrap_or(sib);
                    println!("  → 父级下一个: {}", sib_title.cyan());
                    break;
                }
            }
        }
    }

    // Record progress
    let mut progress = ledger::Progress::load(&cards_dir, content_id);
    progress.record_parent();
    let _ = progress.save(&cards_dir, content_id);

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read sibling — tail text of preceding sibling
// ══════════════════════════════════════════════════════════════════

pub fn read_sibling(project: &Path, content_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let tree = ContentTree::load(&everlore);

    if !tree.nodes.contains_key(content_id) {
        return Err(format!("Content '{content_id}' 不存在").into());
    }

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let parent_id = match tree.parents.get(content_id) {
        Some(p) => p.clone(),
        None => {
            println!("{}", "(根节点，没有前序兄弟)".dimmed());
            return Ok(());
        }
    };

    let siblings = tree.children_of(&parent_id);

    // Find preceding sibling
    let mut prev_sib: Option<&str> = None;
    for sib_id in siblings {
        if sib_id == content_id {
            break;
        }
        prev_sib = Some(sib_id.as_str());
    }

    let Some(prev_id) = prev_sib else {
        println!("{}", "(没有前序兄弟 — 这是第一个子节点)".dimmed());
        return Ok(());
    };

    // Get the last leaf of the preceding sibling's subtree
    let last_leaf = if tree.is_branch(prev_id) {
        let leaves = tree.subtree_leaves(prev_id);
        leaves.last().cloned()
    } else {
        Some(prev_id.to_string())
    };

    let Some(leaf_id) = last_leaf else {
        println!("{}", "(前序兄弟没有叶子内容)".dimmed());
        return Ok(());
    };

    let prev_title = content_map.get(prev_id)
        .and_then(|c| c.title.as_deref())
        .unwrap_or(prev_id);

    let leaf_content = content_map.get(leaf_id.as_str());
    let leaf_title = leaf_content.and_then(|c| c.title.as_deref()).unwrap_or(&leaf_id);

    // Show header
    if prev_id == leaf_id {
        println!(
            "── {} ({}) ──",
            prev_title.bold(),
            prev_id.dimmed()
        );
    } else {
        println!(
            "── {} → {} ──",
            prev_title.bold(),
            leaf_title.bold()
        );
    }

    // Show the tail of the leaf text (last ~5 paragraphs)
    if let Some(content) = leaf_content {
        let clean = strip_refs(&content.text);
        let paragraphs: Vec<&str> = clean.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
        let total = paragraphs.len();
        let tail: Vec<&str> = if total > 5 {
            paragraphs[total - 5..].to_vec()
        } else {
            paragraphs
        };

        if tail.is_empty() {
            println!("{}", "(无文本)".dimmed());
        } else {
            if total > 5 {
                println!("{}", "  …".dimmed());
            }
            println!();
            for para in &tail {
                println!("  {}", para.dimmed());
                println!();
            }
        }

        // Show synopsis of current node for reference
        let current = content_map.get(content_id);
        let current_title = current.and_then(|c| c.title.as_deref()).unwrap_or(content_id);
        println!("── {} {} ──", "→".cyan(), current_title.bold());
        if let Some(synopsis) = current.and_then(|c| c.synopsis.as_deref()) {
            println!("  {}", synopsis.dimmed());
        }
    }

    // Record progress
    let mut progress = ledger::Progress::load(&cards_dir, content_id);
    progress.record_sibling();
    let _ = progress.save(&cards_dir, content_id);

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// read pov — view POV drafts for a content node
// ══════════════════════════════════════════════════════════════════

pub fn read_pov(project: &Path, content_id: &str, who: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let drafts = ledger::load_pov_drafts(&cards_dir, content_id);

    if drafts.is_empty() {
        let node_dir = ledger::card::content_node_dir(&cards_dir, content_id);
        println!("{}", format!("({content_id} 没有视角草稿)").dimmed());
        println!();
        println!("  在以下目录创建 .md 文件:");
        println!("    {}/pov/       ← 角色视角", node_dir.display());
        println!("    {}/timeline/  ← 时间轴视角", node_dir.display());
        println!("    {}/outsider/  ← 路人视角", node_dir.display());
        return Ok(());
    }

    let filtered: Vec<_> = match who {
        Some(w) => drafts.iter().filter(|d| d.name == w || d.category.dir_name() == w).collect(),
        None => drafts.iter().collect(),
    };

    if filtered.is_empty() {
        println!("{}", format!("({content_id} 没有匹配 '{}' 的草稿)", who.unwrap_or("?")).dimmed());
        return Ok(());
    }

    for draft in &filtered {
        let chars = draft.text.chars().count();
        println!(
            "── [{}] {} ({} 字) ──",
            draft.category.dir_name(),
            draft.name.bold(),
            chars
        );
        println!();
        let clean = strip_refs(&draft.text);
        println!("{}", clean);
        println!();
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// context — entity narrative context (effects + text refs)
// ══════════════════════════════════════════════════════════════════

/// A context hit from a content leaf.
struct ContextHit {
    content_id: String,
    title: String,
    /// Strong = effects reference this entity, Weak = text link reference
    strong: bool,
    /// For strong hits: the relevant effects
    effects: Vec<String>,
    /// For weak hits: surrounding text context
    text_snippets: Vec<String>,
    /// Full text (for --full mode)
    full_text: String,
}

pub fn context(project: &Path, entity_id: &str, full: bool) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");
    let entities_dir = everlore.join("entities");
    let tree = ContentTree::load(&everlore);

    let content_map: BTreeMap<String, Content> = card::load_content_cards(&cards_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    // Determine cursor position: active node, or last leaf
    let cursor = tree.active.as_deref()
        .or(tree.leaves_dfs().last().map(|s| s.as_str()))
        .unwrap_or("")
        .to_string();

    // ── Section 1: State from snapshot ──
    let snap = Snapshot::at_content(
        &cursor, &tree, &content_map, &entities_dir, &cards_dir, &everlore,
    );

    let cursor_title = content_map.get(cursor.as_str())
        .and_then(|c| c.title.as_deref())
        .unwrap_or(&cursor);

    println!(
        "{}",
        format!("═══ Context: {entity_id} @{cursor_title} ═══").cyan().bold(),
    );
    println!();

    if let Ok(ref snap) = snap {
        // Find entity in snapshot
        let entity = snap.entities.iter().find(|e| e.id() == entity_id);
        if let Some(entity) = entity {
            println!("{}", "[状态]".cyan());
            if let Some(c) = entity.as_character() {
                if !c.traits.is_empty() {
                    println!("  traits: {}", c.traits.join(", ").yellow());
                }
                if let Some(ref loc) = c.location {
                    println!("  location: {}", loc.cyan());
                }
                if !c.beliefs.is_empty() {
                    println!("  beliefs: {}", c.beliefs.join(", "));
                }
                if !c.desires.is_empty() {
                    println!("  desires: {}", c.desires.join(", "));
                }
            }
            println!();
        }

        // Secrets: known vs unknown
        let known: Vec<&ledger::Secret> = snap.secrets.iter()
            .filter(|s| s.known_by.contains(&entity_id.to_string()))
            .collect();
        let unknown: Vec<&ledger::Secret> = snap.secrets.iter()
            .filter(|s| !s.known_by.contains(&entity_id.to_string()))
            .collect();

        if !known.is_empty() || !unknown.is_empty() {
            println!("{}", "[信息]".cyan());
            for s in &known {
                println!("  {} {} — {}", "✓".green(), s.id.bold(), s.content.dimmed());
            }
            for s in &unknown {
                let irony = if s.revealed_to_reader { " ← 戏剧反讽" } else { "" };
                println!("  {} {} — {}{}", "✗".red(), s.id.bold(), s.content.dimmed(), irony.yellow());
            }
            println!();
        }
    }

    // ── Section 2: Trajectory (hits from content) ──
    let leaves = tree.leaves_dfs();
    let mut hits: Vec<ContextHit> = Vec::new();

    // Only include leaves up to and including cursor position
    let cursor_leaves: Vec<&String> = {
        let replay = tree.replay_path(&cursor);
        leaves.iter().filter(|id| replay.contains(id)).collect()
    };

    for leaf_id in &cursor_leaves {
        let Some(content) = content_map.get(leaf_id.as_str()) else {
            continue;
        };

        let title = content.title.as_deref().unwrap_or(leaf_id).to_string();

        // Strong: effects reference this entity
        let matching_effects: Vec<String> = content
            .effects
            .iter()
            .filter(|op| op.all_referenced_ids().iter().any(|id| *id == entity_id))
            .map(|op| op.describe())
            .collect();

        // Weak: markdown links in text
        let refs = extract_refs(&content.text);
        let matching_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.entity_id == entity_id)
            .collect();

        let text_snippets: Vec<String> = matching_refs
            .iter()
            .map(|r| {
                let raw = extract_context(&content.text, r.offset, 1);
                strip_refs(&raw)
            })
            .collect();

        if !matching_effects.is_empty() || !matching_refs.is_empty() {
            hits.push(ContextHit {
                content_id: leaf_id.to_string(),
                title,
                strong: !matching_effects.is_empty(),
                effects: matching_effects,
                text_snippets,
                full_text: content.text.clone(),
            });
        }
    }

    if hits.is_empty() {
        println!("{}", "(没有叙事轨迹)".dimmed());
        return Ok(());
    }

    println!("{} {}", "[轨迹]".cyan(), format!("({} hits)", hits.len()).dimmed());

    for hit in &hits {
        let assoc = if hit.strong {
            "effects".green().bold().to_string()
        } else {
            "ref".yellow().to_string()
        };
        println!(
            "  ── {} ({}) ── {} ──",
            hit.title.bold(),
            hit.content_id.dimmed(),
            assoc
        );

        if !hit.effects.is_empty() {
            println!(
                "    effects: {}",
                hit.effects.join(", ").yellow()
            );
        }

        if full {
            if !hit.full_text.is_empty() {
                println!();
                let clean = strip_refs(&hit.full_text);
                for line in clean.lines() {
                    println!("    {}", line);
                }
            }
        } else if !hit.text_snippets.is_empty() {
            for snippet in &hit.text_snippets {
                for line in snippet.lines() {
                    println!("    {}", line.dimmed());
                }
            }
        }

        println!();
    }

    // Record progress for cursor node
    if !cursor.is_empty() {
        let mut progress = ledger::Progress::load(&cards_dir, &cursor);
        progress.record_context(entity_id);
        let _ = progress.save(&cards_dir, &cursor);
    }

    Ok(())
}
