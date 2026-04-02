//! `elore publish` — compile the content tree into a readable Markdown document.
//!
//! Traverses the tree in DFS pre-order, emitting each node's title as a
//! heading (depth-based: # / ## / ### ...) followed by its text body.

use std::collections::BTreeMap;
use std::path::Path;

use colored::Colorize;

use ledger::card;
use ledger::state::content::{Content, ContentTree};
use ledger::strip_refs;

pub fn run(
    project: &Path,
    output: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cards_dir = project.join("cards");
    let everlore = project.join(".everlore");

    let tree = ContentTree::load(&everlore);
    let Some(ref root) = tree.root else {
        println!(
            "{}",
            "(没有内容树 — 请先在 cards/content/ 创建节点后运行 `elore build`)"
                .yellow()
        );
        return Ok(());
    };

    let contents = card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> = contents
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();

    let mut doc = String::new();
    let mut total_words = 0u32;
    let mut node_count = 0u32;

    emit_node(&tree, &content_map, root, 1, &mut doc, &mut total_words, &mut node_count);

    if let Some(out_path) = output {
        std::fs::write(out_path, &doc)?;
        println!(
            "{} 已导出 → {} ({} 节点, {} 字)",
            "✓".green().bold(),
            out_path.display(),
            node_count,
            total_words,
        );
    } else {
        print!("{doc}");
    }

    Ok(())
}

fn emit_node(
    tree: &ContentTree,
    content_map: &BTreeMap<String, Content>,
    node_id: &str,
    depth: usize,
    doc: &mut String,
    total_words: &mut u32,
    node_count: &mut u32,
) {
    let Some(content) = content_map.get(node_id) else {
        return;
    };

    let is_branch = tree.is_branch(node_id);

    // Heading: use title if available, otherwise id
    let title = content
        .title
        .as_deref()
        .unwrap_or(&content.id);

    // Depth → heading level (capped at 6)
    let level = depth.min(6);
    let hashes = "#".repeat(level);

    // Branch nodes emit heading only; leaves emit heading + text
    if is_branch {
        doc.push_str(&format!("{hashes} {title}\n\n"));
    } else {
        *node_count += 1;
        *total_words += content.word_count;
        doc.push_str(&format!("{hashes} {title}\n\n"));
        if !content.text.is_empty() {
            doc.push_str(&strip_refs(&content.text));
            doc.push_str("\n\n");
        }
    }

    // Recurse into children
    for child_id in tree.children_of(node_id) {
        emit_node(tree, content_map, child_id, depth + 1, doc, total_words, node_count);
    }
}
