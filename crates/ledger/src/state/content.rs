//! Content tree — fractal narrative structure (B-tree model v2).
//!
//! A `Content` node is the atomic unit of the narrative tree.
//! - **Leaf** nodes have text + effects and produce narrative content.
//! - **Branch** nodes have children; their effects are distributed to children as slots.
//!
//! Branch vs leaf is determined by tree structure (has children = branch),
//! not by a stored flag.
//!
//! Snapshot replay only visits leaf nodes in DFS pre-order.
//!
//! Unlock rule: a node can be Active if its parent is Committed OR if its
//! parent has children (i.e., parent is being expanded as a branch).
//! Preceding siblings must still be committed.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;
use crate::effect::op::Op;
use crate::state::content_constraint::ContentConstraints;

// ══════════════════════════════════════════════════════════════════
// Content status
// ══════════════════════════════════════════════════════════════════

/// Lifecycle of a content node.
///
/// ```text
/// locked → active → committed
///            ↑          ↓
///            └── (reject)
/// ```
///
/// - **Locked**: parent not ready, or preceding sibling not committed.
/// - **Active**: ready to be worked on.
/// - **Committed**: finalized (L1–L4 passed for leaves; slot coverage for branches).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentStatus {
    #[default]
    Locked,
    Active,
    Committed,
}

// ══════════════════════════════════════════════════════════════════
// Content node
// ══════════════════════════════════════════════════════════════════

/// A single node in the content tree (parsed from card file).
///
/// Whether this node is a leaf or branch is determined by the tree structure,
/// not by any field on this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub id: String,

    /// Parent content ID. `None` = root node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Ordering among siblings (1-indexed).
    #[serde(default = "default_order")]
    pub order: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synopsis: Option<String>,

    /// Narrative text (Markdown body of the card). Only meaningful for leaves.
    #[serde(default)]
    pub text: String,

    /// State mutations produced by this node.
    /// For leaves: the actual effects applied during snapshot replay.
    /// For branches: the "budget" that must be fully covered by children's inherited_slots.
    #[serde(default)]
    pub effects: Vec<Op>,

    /// Word count (CJK + Latin).
    #[serde(default)]
    pub word_count: u32,

    /// Node-level constraints (flat, no L1/L2/L3 wrappers).
    #[serde(default)]
    pub constraints: ContentConstraints,

    /// Main POV character for this node. Inherited from ancestors if not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_role: Option<String>,

    /// Style directives for this node. Inherited from ancestors unless `style_override` is true.
    #[serde(default)]
    pub style: Vec<String>,

    /// If true, discard all inherited styles and only use this node's `style`.
    #[serde(default)]
    pub style_override: bool,

    #[serde(default = "default_creator")]
    pub created_by: String,

    #[serde(default)]
    pub created_at: String,
}

fn default_order() -> u32 {
    1
}

fn default_creator() -> String {
    "human".to_string()
}

impl Content {
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

// ══════════════════════════════════════════════════════════════════
// Content tree
// ══════════════════════════════════════════════════════════════════

/// Runtime state for a content node in the tree.
///
/// Minimal: only lifecycle state. Display data (words, effects) comes from Content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentEntry {
    pub status: ContentStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_at: Option<String>,

    /// Version number. Incremented each time a committed node is re-activated.
    #[serde(default = "default_version")]
    pub version: u32,

    /// True if an ancestor was re-edited after this node was committed.
    /// Indicates the snapshot may be out of date.
    #[serde(default)]
    pub stale: bool,
}

fn default_version() -> u32 {
    1
}

/// The content tree — manages the tree structure and node lifecycle.
///
/// Persisted as `.everlore/content_tree.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentTree {
    /// Root content ID (None if tree is empty).
    #[serde(default)]
    pub root: Option<String>,

    /// Currently active content node.
    #[serde(default)]
    pub active: Option<String>,

    /// Per-node runtime state.
    #[serde(default)]
    pub nodes: BTreeMap<String, ContentEntry>,

    /// Parent → ordered children mapping (derived, kept in sync).
    #[serde(default)]
    pub children: BTreeMap<String, Vec<String>>,

    /// Node → parent mapping.
    #[serde(default)]
    pub parents: BTreeMap<String, String>,
}

const TREE_FILE: &str = "content_tree.json";

impl ContentTree {
    // ── Load / Save ──────────────────────────────────────────────

    pub fn load(everlore_dir: &Path) -> Self {
        let path = everlore_dir.join(TREE_FILE);
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, everlore_dir: &Path) -> Result<(), LedgerError> {
        std::fs::create_dir_all(everlore_dir)?;
        let path = everlore_dir.join(TREE_FILE);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    // ── Tree construction ────────────────────────────────────────

    /// Register a content node from its definition.
    /// Determines initial status based on parent/sibling state.
    pub fn register(&mut self, content: &Content) {
        if content.is_root() {
            self.root = Some(content.id.clone());
        }

        if let Some(ref parent_id) = content.parent {
            self.parents
                .insert(content.id.clone(), parent_id.clone());
            let siblings = self.children.entry(parent_id.clone()).or_default();
            if !siblings.contains(&content.id) {
                siblings.push(content.id.clone());
            }
        }

        let status = self.compute_status(&content.id, content.parent.as_deref());

        self.nodes.entry(content.id.clone()).or_insert(ContentEntry {
            status,
            committed_at: None,
            version: 1,
            stale: false,
        });
    }

    /// Sort children lists using a map of content_id → order.
    pub fn sort_children(&mut self, orders: &BTreeMap<String, u32>) {
        for children in self.children.values_mut() {
            children.sort_by_key(|id| orders.get(id).copied().unwrap_or(u32::MAX));
        }
    }

    /// Compute the correct status for a node.
    fn compute_status(&self, id: &str, parent: Option<&str>) -> ContentStatus {
        match parent {
            None => ContentStatus::Active, // root is always unlocked
            Some(parent_id) => {
                // Parent must be committed, OR actively being expanded (Active + has children)
                let parent_status = self.nodes.get(parent_id).map(|e| &e.status);
                let parent_ready = match parent_status {
                    Some(ContentStatus::Committed) => true,
                    Some(ContentStatus::Active) => self.is_branch(parent_id),
                    _ => false, // Locked parent → children locked
                };

                if !parent_ready {
                    return ContentStatus::Locked;
                }

                // All preceding siblings must be committed
                if let Some(siblings) = self.children.get(parent_id) {
                    for sib_id in siblings {
                        if sib_id == id {
                            break;
                        }
                        let sib_committed = self
                            .nodes
                            .get(sib_id)
                            .is_some_and(|e| e.status == ContentStatus::Committed);
                        if !sib_committed {
                            return ContentStatus::Locked;
                        }
                    }
                }

                ContentStatus::Active
            }
        }
    }

    // ── Lifecycle transitions ────────────────────────────────────

    /// Move the cursor to a content node. Pure cursor movement — does NOT change status.
    pub fn activate(&mut self, content_id: &str) -> Result<(), LedgerError> {
        let entry = self
            .nodes
            .get(content_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Content '{content_id}' 未注册")))?;

        if entry.status == ContentStatus::Locked {
            return Err(LedgerError::Other(format!(
                "Content '{content_id}' 仍 locked — 父节点或前序兄弟未 committed"
            )));
        }

        self.active = Some(content_id.to_string());
        Ok(())
    }

    /// Re-open a committed node for editing. Bumps version, marks downstream stale.
    /// No-op if already Active. Returns downstream node IDs marked stale.
    pub fn edit(&mut self, content_id: &str) -> Result<Vec<String>, LedgerError> {
        let entry = self
            .nodes
            .get(content_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Content '{content_id}' 未注册")))?;

        match entry.status {
            ContentStatus::Locked => {
                return Err(LedgerError::Other(format!(
                    "Content '{content_id}' 仍 locked"
                )));
            }
            ContentStatus::Active => {
                self.active = Some(content_id.to_string());
                return Ok(Vec::new());
            }
            ContentStatus::Committed => {}
        }

        let entry = self.nodes.get_mut(content_id).unwrap();
        entry.status = ContentStatus::Active;
        entry.version += 1;
        self.active = Some(content_id.to_string());

        let downstream = self.downstream_of(content_id);
        for nid in &downstream {
            if let Some(e) = self.nodes.get_mut(nid.as_str()) {
                e.stale = true;
            }
        }
        Ok(downstream)
    }

    /// Commit a content node. Transitions active → committed.
    /// Caller must verify constraints before calling this.
    pub fn commit(&mut self, content_id: &str) -> Result<(), LedgerError> {
        let entry = self
            .nodes
            .get_mut(content_id)
            .ok_or_else(|| LedgerError::NotFound(format!("Content '{content_id}' 未注册")))?;

        if entry.status != ContentStatus::Active {
            return Err(LedgerError::Other(format!(
                "Content '{content_id}' 不是 active 状态，不能 commit"
            )));
        }

        entry.status = ContentStatus::Committed;
        entry.committed_at = Some(now());

        if self.active.as_deref() == Some(content_id) {
            self.active = None;
        }

        self.resolve_locks();
        Ok(())
    }

    /// Re-evaluate all locked nodes and unlock those whose deps are met.
    pub fn resolve_locks(&mut self) {
        let locked_ids: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, e)| e.status == ContentStatus::Locked)
            .map(|(id, _)| id.clone())
            .collect();

        for id in locked_ids {
            let parent = self.parents.get(&id).cloned();
            let new_status = self.compute_status(&id, parent.as_deref());
            if new_status != ContentStatus::Locked {
                if let Some(entry) = self.nodes.get_mut(&id) {
                    entry.status = new_status;
                }
            }
        }
    }

    // ── Tree queries ─────────────────────────────────────────────

    /// Get ancestor chain from root to the given node (inclusive).
    pub fn ancestors(&self, content_id: &str) -> Vec<String> {
        let mut chain = vec![content_id.to_string()];
        let mut current = content_id;
        while let Some(parent) = self.parents.get(current) {
            chain.push(parent.clone());
            current = parent;
        }
        chain.reverse();
        chain
    }

    /// Get ordered children of a node.
    pub fn children_of(&self, content_id: &str) -> &[String] {
        self.children
            .get(content_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Whether a node has children (= branch in B-tree model).
    pub fn is_branch(&self, content_id: &str) -> bool {
        !self.children_of(content_id).is_empty()
    }

    /// Whether a node has no children (= leaf in B-tree model).
    pub fn is_leaf(&self, content_id: &str) -> bool {
        self.children_of(content_id).is_empty()
    }

    /// DFS pre-order traversal of the entire tree.
    pub fn dfs_order(&self) -> Vec<String> {
        let Some(ref root) = self.root else {
            return Vec::new();
        };
        let mut result = Vec::new();
        self.dfs_walk(root, &mut result);
        result
    }

    fn dfs_walk(&self, node: &str, result: &mut Vec<String>) {
        result.push(node.to_string());
        for child in self.children_of(node) {
            self.dfs_walk(child, result);
        }
    }

    /// All nodes that come after `content_id` in DFS order (downstream dependents).
    pub fn downstream_of(&self, content_id: &str) -> Vec<String> {
        let full = self.dfs_order();
        let mut found = false;
        let mut result = Vec::new();
        for id in &full {
            if found {
                result.push(id.clone());
            }
            if id == content_id {
                found = true;
            }
        }
        result
    }

    /// DFS path up to and INCLUDING the given node.
    pub fn replay_path(&self, content_id: &str) -> Vec<String> {
        let full = self.dfs_order();
        let mut path = Vec::new();
        for id in full {
            path.push(id.clone());
            if id == content_id {
                break;
            }
        }
        path
    }

    /// DFS path up to but NOT including the given node.
    pub fn replay_path_before(&self, content_id: &str) -> Vec<String> {
        let full = self.dfs_order();
        let mut path = Vec::new();
        for id in full {
            if id == content_id {
                break;
            }
            path.push(id);
        }
        path
    }

    /// Get preceding siblings and their subtrees.
    pub fn preceding_subtrees(&self, content_id: &str) -> Vec<String> {
        let parent = match self.parents.get(content_id) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let siblings = self.children_of(parent);
        let mut result = Vec::new();
        for sib in siblings {
            if sib == content_id {
                break;
            }
            self.dfs_walk(sib, &mut result);
        }
        result
    }

    /// Check if all children of a node are committed.
    pub fn all_children_committed(&self, content_id: &str) -> bool {
        self.children_of(content_id).iter().all(|child| {
            self.nodes
                .get(child)
                .is_some_and(|e| e.status == ContentStatus::Committed)
        })
    }

    /// Get all content IDs in the tree.
    pub fn all_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Depth of a node (root = 0).
    pub fn depth(&self, content_id: &str) -> usize {
        let mut d = 0;
        let mut current = content_id;
        while let Some(parent) = self.parents.get(current) {
            d += 1;
            current = parent;
        }
        d
    }

    /// Get all nodes at a specific depth.
    pub fn nodes_at_depth(&self, target_depth: usize) -> Vec<String> {
        self.dfs_order()
            .into_iter()
            .filter(|id| self.depth(id) == target_depth)
            .collect()
    }

    // ── B-tree leaf methods ─────────────────────────────────────

    /// Get all leaf nodes in DFS pre-order.
    pub fn leaves_dfs(&self) -> Vec<String> {
        self.dfs_order()
            .into_iter()
            .filter(|id| self.is_leaf(id))
            .collect()
    }

    /// DFS path through LEAVES ONLY, up to and INCLUDING the given node.
    /// Used for snapshot replay — only leaves carry effects.
    pub fn leaf_replay_path(&self, content_id: &str) -> Vec<String> {
        let full = self.dfs_order();
        let mut path = Vec::new();
        for id in full {
            if self.is_leaf(&id) {
                path.push(id.clone());
            }
            if id == content_id {
                break;
            }
        }
        path
    }

    /// DFS path through LEAVES ONLY, up to but NOT including the given node.
    pub fn leaf_replay_path_before(&self, content_id: &str) -> Vec<String> {
        let full = self.dfs_order();
        let mut path = Vec::new();
        for id in full {
            if id == content_id {
                break;
            }
            if self.is_leaf(&id) {
                path.push(id);
            }
        }
        path
    }

    /// Get all leaves within a specific node's subtree (DFS order).
    pub fn subtree_leaves(&self, content_id: &str) -> Vec<String> {
        let mut all = Vec::new();
        self.dfs_walk(content_id, &mut all);
        all.into_iter()
            .filter(|id| self.is_leaf(id))
            .collect()
    }
}

/// Check slot coverage: each parent effect should appear in at least one child's effects.
///
/// Returns Ok if all covered, Err with uncovered effects.
/// Uncovered effects are not fatal — they're applied by the parent during replay.
/// This is informational for branch commit validation.
pub fn check_slot_coverage(
    parent_effects: &[Op],
    children_effects: &[Vec<Op>],
) -> (Vec<String>, Vec<String>) {
    let mut covered = Vec::new();
    let mut uncovered = Vec::new();

    for op in parent_effects {
        let found = children_effects.iter().any(|child| child.contains(op));
        if found {
            covered.push(op.describe());
        } else {
            uncovered.push(op.describe());
        }
    }

    (covered, uncovered)
}

/// Compute the effective style for a node by walking the ancestor chain.
///
/// Accumulates style directives from root → node. If a node has `style_override: true`,
/// all inherited styles are discarded and only that node's (and its descendants') styles apply.
pub fn effective_style(
    content_id: &str,
    tree: &ContentTree,
    contents: &BTreeMap<String, Content>,
) -> Vec<String> {
    let ancestors = tree.ancestors(content_id);
    let mut styles = Vec::new();
    for id in &ancestors {
        if let Some(content) = contents.get(id.as_str()) {
            if content.style_override {
                styles.clear();
            }
            styles.extend(content.style.iter().cloned());
        }
    }
    styles
}

/// Resolve effective main_role for a content node (inherits from ancestors).
///
/// Walks up the ancestor chain and returns the first `main_role` found.
pub fn effective_main_role(
    content_id: &str,
    tree: &ContentTree,
    contents: &BTreeMap<String, Content>,
) -> Option<String> {
    let ancestors = tree.ancestors(content_id);
    for id in ancestors.iter().rev() {
        if let Some(content) = contents.get(id.as_str()) {
            if content.main_role.is_some() {
                return content.main_role.clone();
            }
        }
    }
    None
}

// ══════════════════════════════════════════════════════════════════
// Markdown link extraction for context indexing
// ══════════════════════════════════════════════════════════════════

/// A reference link found in content text: `[display](card_type/entity_id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentRef {
    /// The display text (e.g., "小晚", "她", "书店")
    pub display: String,
    /// The card path (e.g., "characters/lin", "locations/bookstore")
    pub card_path: String,
    /// The entity id extracted from card_path (e.g., "lin", "bookstore")
    pub entity_id: String,
    /// Byte offset in the source text
    pub offset: usize,
}

/// Extract all `[text](card_type/entity_id)` links from content text.
pub fn extract_refs(text: &str) -> Vec<ContentRef> {
    let mut refs = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' {
            let link_start = i;
            // Find closing ]
            let Some(bracket_end) = text[i + 1..].find(']').map(|p| i + 1 + p) else {
                i += 1;
                continue;
            };
            // Must be followed by (
            if bracket_end + 1 >= len || bytes[bracket_end + 1] != b'(' {
                i = bracket_end + 1;
                continue;
            }
            // Find closing )
            let Some(paren_end) = text[bracket_end + 2..].find(')').map(|p| bracket_end + 2 + p) else {
                i = bracket_end + 1;
                continue;
            };

            let display = &text[link_start + 1..bracket_end];
            let card_path = &text[bracket_end + 2..paren_end];

            // Must contain a / to be a card reference (card_type/entity_id)
            if let Some(slash_pos) = card_path.rfind('/') {
                let entity_id = &card_path[slash_pos + 1..];
                if !entity_id.is_empty() && !display.is_empty() {
                    refs.push(ContentRef {
                        display: display.to_string(),
                        card_path: card_path.to_string(),
                        entity_id: entity_id.to_string(),
                        offset: link_start,
                    });
                }
            }

            i = paren_end + 1;
        } else {
            i += 1;
        }
    }

    refs
}

/// Strip card reference links from text, keeping only the display text.
/// `[小晚](characters/lin)走了` → `小晚走了`.
/// Non-card links (no `/` in path) are left intact.
pub fn strip_refs(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' {
            let link_start = i;
            let Some(bracket_end) = text[i + 1..].find(']').map(|p| i + 1 + p) else {
                result.push('[');
                i += 1;
                continue;
            };
            if bracket_end + 1 >= len || bytes[bracket_end + 1] != b'(' {
                result.push_str(&text[link_start..=bracket_end]);
                i = bracket_end + 1;
                continue;
            }
            let Some(paren_end) = text[bracket_end + 2..].find(')').map(|p| bracket_end + 2 + p) else {
                result.push_str(&text[link_start..=bracket_end + 1]);
                i = bracket_end + 2;
                continue;
            };

            let display = &text[link_start + 1..bracket_end];
            let card_path = &text[bracket_end + 2..paren_end];

            if card_path.contains('/') && !display.is_empty() {
                // Card reference — emit display text only
                result.push_str(display);
            } else {
                // Not a card reference — keep as-is
                result.push_str(&text[link_start..=paren_end]);
            }
            i = paren_end + 1;
        } else {
            result.push(text[i..].chars().next().unwrap());
            i += text[i..].chars().next().unwrap().len_utf8();
        }
    }

    result
}

/// Extract surrounding context lines around a match offset in text.
/// Returns `context_lines` lines before and after the line containing `offset`.
pub fn extract_context(text: &str, offset: usize, context_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    // Find which line the offset falls on
    let mut cumulative = 0;
    let mut target_line = 0;
    for (i, line) in lines.iter().enumerate() {
        let line_end = cumulative + line.len() + 1; // +1 for newline
        if offset < line_end {
            target_line = i;
            break;
        }
        cumulative = line_end;
    }

    let start = target_line.saturating_sub(context_lines);
    let end = (target_line + context_lines + 1).min(lines.len());
    lines[start..end].join("\n")
}

fn now() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", d.as_secs())
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_content(id: &str, parent: Option<&str>, order: u32) -> Content {
        Content {
            id: id.into(),
            parent: parent.map(String::from),
            order,
            title: None,
            synopsis: None,
            text: String::new(),
            effects: vec![],
            word_count: 0,
            constraints: ContentConstraints::default(),
            main_role: None,
            style: vec![],
            style_override: false,
            created_by: "test".into(),
            created_at: String::new(),
        }
    }

    fn build_test_tree() -> ContentTree {
        // Tree structure:
        //   root
        //   ├── a (order 1)
        //   │   ├── a1 (order 1)
        //   │   └── a2 (order 2)
        //   └── b (order 2)
        //       └── b1 (order 1)
        let mut tree = ContentTree::default();
        let nodes = vec![
            make_content("root", None, 1),
            make_content("a", Some("root"), 1),
            make_content("b", Some("root"), 2),
            make_content("a1", Some("a"), 1),
            make_content("a2", Some("a"), 2),
            make_content("b1", Some("b"), 1),
        ];

        let mut orders: BTreeMap<String, u32> = BTreeMap::new();
        for n in &nodes {
            orders.insert(n.id.clone(), n.order);
            tree.register(n);
        }
        tree.sort_children(&orders);
        tree
    }

    #[test]
    fn tree_structure() {
        let tree = build_test_tree();
        assert_eq!(tree.root.as_deref(), Some("root"));
        assert_eq!(tree.children_of("root"), &["a", "b"]);
        assert_eq!(tree.children_of("a"), &["a1", "a2"]);
        assert_eq!(tree.children_of("b"), &["b1"]);
        assert!(tree.is_leaf("a1"));
        assert!(tree.is_leaf("b1"));
        assert!(tree.is_branch("root"));
        assert!(tree.is_branch("a"));
    }

    #[test]
    fn dfs_order() {
        let tree = build_test_tree();
        assert_eq!(
            tree.dfs_order(),
            vec!["root", "a", "a1", "a2", "b", "b1"]
        );
    }

    #[test]
    fn replay_path() {
        let tree = build_test_tree();
        assert_eq!(
            tree.replay_path("a2"),
            vec!["root", "a", "a1", "a2"]
        );
        assert_eq!(
            tree.replay_path("b1"),
            vec!["root", "a", "a1", "a2", "b", "b1"]
        );
        assert_eq!(tree.replay_path("root"), vec!["root"]);
    }

    #[test]
    fn ancestors() {
        let tree = build_test_tree();
        assert_eq!(tree.ancestors("a2"), vec!["root", "a", "a2"]);
        assert_eq!(tree.ancestors("b1"), vec!["root", "b", "b1"]);
        assert_eq!(tree.ancestors("root"), vec!["root"]);
    }

    #[test]
    fn branch_children_unlocked_when_parent_active() {
        let tree = build_test_tree();
        // Root is Active (not committed), but has children → branch → children unlock
        assert_eq!(tree.nodes["root"].status, ContentStatus::Active);
        // a should be Active (root is branch, a is first child)
        assert_eq!(tree.nodes["a"].status, ContentStatus::Active);
        // b is Locked (preceding sibling a not committed)
        assert_eq!(tree.nodes["b"].status, ContentStatus::Locked);
        // a1 should be Active (a is branch, a1 is first child)
        assert_eq!(tree.nodes["a1"].status, ContentStatus::Active);
    }

    #[test]
    fn commit_unlocks_next_sibling() {
        let mut tree = build_test_tree();
        tree.commit("a").unwrap();
        assert_eq!(tree.nodes["b"].status, ContentStatus::Active);
    }

    #[test]
    fn sibling_order_matters() {
        let mut tree = build_test_tree();
        // a1 is active (first child of branch a)
        assert_eq!(tree.nodes["a1"].status, ContentStatus::Active);
        // a2 is locked (preceding sibling a1 not committed)
        assert_eq!(tree.nodes["a2"].status, ContentStatus::Locked);

        tree.commit("a1").unwrap();
        assert_eq!(tree.nodes["a2"].status, ContentStatus::Active);
    }

    #[test]
    fn cannot_commit_locked() {
        let mut tree = build_test_tree();
        assert!(tree.commit("b").is_err());
    }

    #[test]
    fn can_activate_committed_for_expansion() {
        let mut tree = build_test_tree();
        tree.commit("root").unwrap();
        // Committed nodes can be activated (cursor) for spawning children
        assert!(tree.activate("root").is_ok());
        assert_eq!(tree.active.as_deref(), Some("root"));
    }

    #[test]
    fn preceding_subtrees() {
        let tree = build_test_tree();
        assert_eq!(tree.preceding_subtrees("b"), vec!["a", "a1", "a2"]);
        assert_eq!(tree.preceding_subtrees("a2"), vec!["a1"]);
        assert!(tree.preceding_subtrees("a").is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let tree = build_test_tree();
        tree.save(dir.path()).unwrap();

        let loaded = ContentTree::load(dir.path());
        assert_eq!(loaded.root, tree.root);
        assert_eq!(loaded.nodes.len(), tree.nodes.len());
        assert_eq!(loaded.children_of("root"), tree.children_of("root"));
    }

    #[test]
    fn all_children_committed() {
        let mut tree = build_test_tree();
        assert!(!tree.all_children_committed("root"));

        tree.commit("a").unwrap();
        tree.commit("b").unwrap();
        assert!(tree.all_children_committed("root"));
    }

    // ── B-tree leaf tests ───────────────────────────────────────

    #[test]
    fn leaves_dfs() {
        let tree = build_test_tree();
        // Branches: root, a, b (they have children)
        // Leaves: a1, a2, b1
        assert_eq!(tree.leaves_dfs(), vec!["a1", "a2", "b1"]);
    }

    #[test]
    fn leaf_replay_path_skips_branches() {
        let tree = build_test_tree();
        // Replay to a2: skip root (branch) and a (branch)
        assert_eq!(tree.leaf_replay_path("a2"), vec!["a1", "a2"]);
        // Replay to b1: skip root, a, b (all branches)
        assert_eq!(tree.leaf_replay_path("b1"), vec!["a1", "a2", "b1"]);
    }

    #[test]
    fn leaf_replay_path_before() {
        let tree = build_test_tree();
        assert_eq!(tree.leaf_replay_path_before("a2"), vec!["a1"]);
        assert_eq!(tree.leaf_replay_path_before("b"), vec!["a1", "a2"]);
    }

    #[test]
    fn subtree_leaves() {
        let tree = build_test_tree();
        assert_eq!(tree.subtree_leaves("a"), vec!["a1", "a2"]);
        assert_eq!(tree.subtree_leaves("root"), vec!["a1", "a2", "b1"]);
        assert_eq!(tree.subtree_leaves("b1"), vec!["b1"]); // leaf is its own subtree leaf
    }

    // ── Slot coverage tests ─────────────────────────────────────

    #[test]
    fn slot_coverage_all_covered() {
        let parent = vec![
            Op::Move { entity: "kian".into(), location: "oasis".into() },
            Op::AddTrait { entity: "kian".into(), value: "brave".into() },
        ];
        let child_a = vec![Op::Move { entity: "kian".into(), location: "oasis".into() }];
        let child_b = vec![Op::AddTrait { entity: "kian".into(), value: "brave".into() }];
        let (_, uncovered) = check_slot_coverage(&parent, &[child_a, child_b]);
        assert!(uncovered.is_empty());
    }

    #[test]
    fn slot_coverage_partial() {
        let parent = vec![
            Op::Move { entity: "kian".into(), location: "oasis".into() },
            Op::AddTrait { entity: "kian".into(), value: "brave".into() },
        ];
        let child_a = vec![Op::Move { entity: "kian".into(), location: "oasis".into() }];
        let (covered, uncovered) = check_slot_coverage(&parent, &[child_a]);
        assert_eq!(covered.len(), 1);
        assert_eq!(uncovered.len(), 1);
    }

    #[test]
    fn slot_coverage_empty_parent() {
        let (_, uncovered) = check_slot_coverage(&[], &[vec![]]);
        assert!(uncovered.is_empty());
    }

    // ── Incremental spawn test ──────────────────────────────────

    #[test]
    fn incremental_spawn_unlocks_children() {
        // Start with just root (leaf)
        let mut tree = ContentTree::default();
        let root = make_content("root", None, 1);
        let mut orders = BTreeMap::new();
        orders.insert("root".to_string(), 1);
        tree.register(&root);
        tree.sort_children(&orders);

        assert!(tree.is_leaf("root"));
        assert_eq!(tree.nodes["root"].status, ContentStatus::Active);

        // Spawn first child — root becomes branch, child unlocks
        let ch1 = make_content("ch1", Some("root"), 1);
        orders.insert("ch1".to_string(), 1);
        tree.register(&ch1);
        tree.sort_children(&orders);
        tree.resolve_locks();

        assert!(tree.is_branch("root"));
        assert_eq!(tree.nodes["root"].status, ContentStatus::Active); // still active!
        assert_eq!(tree.nodes["ch1"].status, ContentStatus::Active); // unlocked!

        // Spawn second child — still works, root still active
        let ch2 = make_content("ch2", Some("root"), 2);
        orders.insert("ch2".to_string(), 2);
        tree.register(&ch2);
        tree.sort_children(&orders);
        tree.resolve_locks();

        assert_eq!(tree.nodes["root"].status, ContentStatus::Active);
        assert_eq!(tree.nodes["ch1"].status, ContentStatus::Active);
        assert_eq!(tree.nodes["ch2"].status, ContentStatus::Locked); // waiting for ch1
    }

    // ── extract_refs tests ──────────────────────────────────────

    #[test]
    fn extract_refs_basic() {
        let text = "[小晚](characters/lin)拖着行李箱走出[车站](locations/station)。";
        let refs = super::extract_refs(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].display, "小晚");
        assert_eq!(refs[0].card_path, "characters/lin");
        assert_eq!(refs[0].entity_id, "lin");
        assert_eq!(refs[1].display, "车站");
        assert_eq!(refs[1].entity_id, "station");
    }

    #[test]
    fn extract_refs_no_slash_ignored() {
        let text = "[链接](https)不是card引用";
        let refs = super::extract_refs(text);
        assert!(refs.is_empty());
    }

    #[test]
    fn extract_refs_pronoun() {
        let text = "[她](characters/lin)微微一笑。";
        let refs = super::extract_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].display, "她");
        assert_eq!(refs[0].entity_id, "lin");
    }

    #[test]
    fn extract_refs_empty_text() {
        let refs = super::extract_refs("");
        assert!(refs.is_empty());
    }

    #[test]
    fn extract_context_middle() {
        let text = "第一行\n第二行\n第三行\n第四行\n第五行";
        // "第三行" starts at byte offset: "第一行\n第二行\n" = 3*3+1+3*3+1 = 20
        let ctx = super::extract_context(text, 20, 1);
        assert!(ctx.contains("第二行"));
        assert!(ctx.contains("第三行"));
        assert!(ctx.contains("第四行"));
    }

    // ── strip_refs tests ────────────────────────────────────────

    #[test]
    fn strip_refs_basic() {
        let text = "[小晚](characters/lin)拖着行李箱走出[车站](locations/station)。";
        assert_eq!(super::strip_refs(text), "小晚拖着行李箱走出车站。");
    }

    #[test]
    fn strip_refs_preserves_non_card_links() {
        let text = "参见[文档](https)和[小晚](characters/lin)。";
        assert_eq!(super::strip_refs(text), "参见[文档](https)和小晚。");
    }

    #[test]
    fn strip_refs_empty() {
        assert_eq!(super::strip_refs(""), "");
    }

    #[test]
    fn strip_refs_no_refs() {
        let text = "没有任何引用的普通文本。";
        assert_eq!(super::strip_refs(text), text);
    }
}
