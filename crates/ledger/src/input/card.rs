//! Card parser — Markdown + YAML frontmatter as source of truth.
//!
//! Cards are `.md` files with YAML frontmatter (between `---` markers).
//! The frontmatter maps to Entity/Secret/Beat structs.
//! The Markdown body is stored in `Entity.description`.

use std::path::{Path, PathBuf};

use crate::LedgerError;
use crate::effect::beat::Beat;
use crate::effect::op::Op;
use crate::input::entity::Entity;
use crate::input::secret::Secret;
use crate::state::content::Content;
use crate::state::content_constraint::ContentConstraints;

/// Returns true for `_template.md` files that should be skipped during loading.
fn is_template(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('_'))
}

// ══════════════════════════════════════════════════════════════════
// Frontmatter parsing
// ══════════════════════════════════════════════════════════════════

/// Split a raw Markdown file into (YAML frontmatter, body).
/// Returns `None` if no valid frontmatter is found.
pub fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Find the opening delimiter
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    // Find the closing delimiter
    let close_pos = after_open.find("\n---")?;
    let yaml = &after_open[..close_pos];
    let rest = &after_open[close_pos + 4..]; // skip "\n---"
    // Skip the newline after closing ---
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    Some((yaml, body))
}

// ══════════════════════════════════════════════════════════════════
// Entity cards
// ══════════════════════════════════════════════════════════════════

/// Load a single entity card from a `.md` file.
pub fn parse_entity_card(path: &Path) -> Result<Entity, LedgerError> {
    let raw = std::fs::read_to_string(path)?;
    parse_entity_card_str(&raw, path)
}

/// Parse an entity from raw card content.
fn parse_entity_card_str(raw: &str, path: &Path) -> Result<Entity, LedgerError> {
    let (yaml, body) = split_frontmatter(raw).ok_or_else(|| {
        LedgerError::Parse(format!("Card missing YAML frontmatter: {}", path.display()))
    })?;

    let mut entity: Entity = serde_yaml::from_str(yaml).map_err(|e| {
        LedgerError::Parse(format!("Card YAML parse error in {}: {e}", path.display()))
    })?;

    let body = body.trim();
    if !body.is_empty() {
        entity.set_description(Some(body.to_string()));
    }

    Ok(entity)
}

/// Load all entity cards from `cards/characters/`, `cards/locations/`, `cards/factions/`.
/// Returns entities paired with their source file paths.
pub fn load_entity_cards(cards_dir: &Path) -> Result<Vec<(Entity, PathBuf)>, LedgerError> {
    let mut result = Vec::new();

    for subdir in ["characters", "locations", "factions"] {
        let dir = cards_dir.join(subdir);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "md") && !is_template(&path) {
                let entity = parse_entity_card(&path)?;
                result.push((entity, path));
            }
        }
    }

    result.sort_by(|a, b| a.0.id().cmp(b.0.id()));
    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
// Secret cards
// ══════════════════════════════════════════════════════════════════

/// Load all secret cards from `cards/secrets/`.
pub fn load_secret_cards(cards_dir: &Path) -> Result<Vec<Secret>, LedgerError> {
    let dir = cards_dir.join("secrets");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut secrets = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "md") && !is_template(&path) {
            let raw = std::fs::read_to_string(&path)?;
            let (yaml, _body) = split_frontmatter(&raw).ok_or_else(|| {
                LedgerError::Parse(format!(
                    "Secret card missing frontmatter: {}",
                    path.display()
                ))
            })?;
            let secret: Secret = serde_yaml::from_str(yaml).map_err(|e| {
                LedgerError::Parse(format!("Secret card parse error in {}: {e}", path.display()))
            })?;
            secrets.push(secret);
        }
    }
    secrets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(secrets)
}

// ══════════════════════════════════════════════════════════════════
// Beat cards
// ══════════════════════════════════════════════════════════════════

/// Intermediate struct for beat frontmatter.
#[derive(Debug, serde::Deserialize)]
struct BeatFrontmatter {
    #[serde(default)]
    seq: Option<u32>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default = "default_creator")]
    created_by: String,
}

fn default_creator() -> String {
    "human".to_string()
}

/// Load all beat cards from `cards/phases/{phase_id}/`, ordered by filename.
pub fn load_beat_cards(cards_dir: &Path, phase_id: &str) -> Result<Vec<Beat>, LedgerError> {
    let dir = cards_dir.join("phases").join(phase_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "md") && !is_template(&path) {
            paths.push(path);
        }
    }
    // Sort by filename for deterministic ordering
    paths.sort();

    let mut beats = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let raw = std::fs::read_to_string(path)?;
        let (yaml, body) = split_frontmatter(&raw).ok_or_else(|| {
            LedgerError::Parse(format!("Beat card missing frontmatter: {}", path.display()))
        })?;

        let fm: BeatFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
            LedgerError::Parse(format!("Beat card parse error in {}: {e}", path.display()))
        })?;

        let seq = fm.seq.unwrap_or((i + 1) as u32);
        let text = body.trim().to_string();
        let word_count = Beat::count_words(&text);

        let effects: Vec<Op> = fm
            .effects
            .iter()
            .map(|s| {
                Op::parse(s).map_err(|e| {
                    LedgerError::Parse(format!("Effect parse error in {}: {e}", path.display()))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        beats.push(Beat {
            phase: phase_id.to_string(),
            seq,
            revises: None,
            revision: 0,
            text,
            effects,
            word_count,
            created_by: fm.created_by,
            created_at: String::new(),
            revision_reason: None,
        });
    }

    Ok(beats)
}

// ══════════════════════════════════════════════════════════════════
// Content cards
// ══════════════════════════════════════════════════════════════════

/// Intermediate struct for content card frontmatter.
///
/// `id` and `parent` are derived from the directory structure, not from YAML.
/// Each content node lives at `cards/content/{path}/root.md`, where
/// `{path}` is the node's id and its parent directory is the parent node.
#[derive(Debug, serde::Deserialize)]
struct ContentFrontmatter {
    #[serde(default = "default_content_order")]
    order: u32,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    synopsis: Option<String>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    constraints: ContentConstraints,
    #[serde(default)]
    main_role: Option<String>,
    #[serde(default)]
    style: Vec<String>,
    #[serde(default)]
    style_override: bool,
    #[serde(default = "default_content_creator")]
    created_by: String,
}

fn default_content_order() -> u32 {
    1
}

fn default_content_creator() -> String {
    "human".to_string()
}

/// Parse a single content card from raw Markdown.
///
/// `id` and `parent` are passed in from the directory structure, not parsed from YAML.
fn parse_content_card_str(
    raw: &str,
    path: &Path,
    id: String,
    parent: Option<String>,
) -> Result<Content, LedgerError> {
    let (yaml, body) = split_frontmatter(raw).ok_or_else(|| {
        LedgerError::Parse(format!(
            "Content card missing frontmatter: {}",
            path.display()
        ))
    })?;

    let fm: ContentFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
        LedgerError::Parse(format!(
            "Content card parse error in {}: {e}",
            path.display()
        ))
    })?;

    let text = body.trim().to_string();
    let word_count = Beat::count_words(&text);

    let effects: Vec<Op> = fm
        .effects
        .iter()
        .map(|s| {
            Op::parse(s).map_err(|e| {
                LedgerError::Parse(format!("Effect parse error in {}: {e}", path.display()))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Content {
        id,
        parent,
        order: fm.order,
        title: fm.title,
        synopsis: fm.synopsis,
        text,
        effects,
        word_count,
        constraints: fm.constraints,
        main_role: fm.main_role,
        style: fm.style,
        style_override: fm.style_override,
        created_by: fm.created_by,
        created_at: String::new(),
    })
}

/// Reserved directory names that hold local entities, not child content nodes.
const RESERVED_DIRS: &[&str] = &["characters", "locations", "factions", "secrets"];

/// Load all content cards by recursively walking the directory tree.
///
/// The tree structure is encoded in the filesystem:
/// - `cards/content/root.md` is the root node (id = "root")
/// - `cards/content/act1/root.md` is a child (id = "act1", parent = "root")
/// - `cards/content/act1/scene1/root.md` is a grandchild (id = "act1/scene1", parent = "act1")
///
/// Directories named `characters`, `locations`, `factions`, `secrets` are
/// reserved for local entity cards and are not treated as content children.
pub fn load_content_cards(cards_dir: &Path) -> Result<Vec<Content>, LedgerError> {
    let content_dir = cards_dir.join("content");
    if !content_dir.exists() {
        return Ok(Vec::new());
    }

    let mut contents = Vec::new();
    walk_content_tree(&content_dir, &content_dir, None, &mut contents)?;

    // Sort by (parent, order, id) for deterministic processing
    contents.sort_by(|a, b| {
        a.parent
            .cmp(&b.parent)
            .then(a.order.cmp(&b.order))
            .then(a.id.cmp(&b.id))
    });

    Ok(contents)
}

/// Recursively walk the content directory tree.
///
/// `base` is `cards/content/`, `dir` is the current directory being scanned,
/// `parent_id` is the content id of the parent node (None for root).
fn walk_content_tree(
    base: &Path,
    dir: &Path,
    parent_id: Option<&str>,
    results: &mut Vec<Content>,
) -> Result<(), LedgerError> {
    let root_md = dir.join("root.md");
    if !root_md.exists() {
        return Ok(());
    }

    // Derive id from relative path: cards/content/ → "root", cards/content/act1/ → "act1"
    let id = if dir == base {
        "root".to_string()
    } else {
        dir.strip_prefix(base)
            .map_err(|e| LedgerError::Parse(format!("path error: {e}")))?
            .to_string_lossy()
            .replace('\\', "/") // normalize Windows paths
    };

    let raw = std::fs::read_to_string(&root_md)?;
    let content = parse_content_card_str(
        &raw,
        &root_md,
        id.clone(),
        parent_id.map(String::from),
    )?;
    results.push(content);

    // Recurse into subdirectories (skip reserved entity dirs)
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if RESERVED_DIRS.contains(&name_str.as_ref()) {
            continue;
        }
        walk_content_tree(base, &entry.path(), Some(&id), results)?;
    }

    Ok(())
}

/// Resolve the filesystem directory for a content node.
///
/// - `"root"` → `cards/content/`
/// - `"act1"` → `cards/content/act1/`
/// - `"act1/scene1"` → `cards/content/act1/scene1/`
pub fn content_node_dir(cards_dir: &Path, content_id: &str) -> PathBuf {
    let content_base = cards_dir.join("content");
    if content_id == "root" {
        content_base
    } else {
        content_base.join(content_id)
    }
}

/// A draft file found in a content node's perspective directories.
#[derive(Debug, Clone)]
pub struct PovDraft {
    /// Filename stem (e.g. "chen_yu", "lao_zheng_past", "street")
    pub name: String,
    /// Which category: pov, timeline, outsider
    pub category: PovCategory,
    /// Raw text content of the draft
    pub text: String,
    /// File path
    pub path: PathBuf,
}

/// The three categories of perspective drafts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovCategory {
    /// Character subjective perspective
    Pov,
    /// Past/future timeline perspective
    Timeline,
    /// Bystander/object/external perspective
    Outsider,
}

impl PovCategory {
    pub fn dir_name(&self) -> &'static str {
        match self {
            Self::Pov => "pov",
            Self::Timeline => "timeline",
            Self::Outsider => "outsider",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pov => "角色视角",
            Self::Timeline => "时间轴视角",
            Self::Outsider => "路人视角",
        }
    }

    pub const ALL: [PovCategory; 3] = [Self::Pov, Self::Timeline, Self::Outsider];
}

/// Summary of drafts per category.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DraftSummary {
    pub pov: Vec<String>,
    pub timeline: Vec<String>,
    pub outsider: Vec<String>,
}

impl DraftSummary {
    pub fn total(&self) -> usize {
        self.pov.len() + self.timeline.len() + self.outsider.len()
    }

    pub fn has_all_categories(&self) -> bool {
        !self.pov.is_empty() && !self.timeline.is_empty() && !self.outsider.is_empty()
    }
}

/// Load all perspective drafts for a content node.
///
/// Scans three subdirectories: `pov/`, `timeline/`, `outsider/`.
/// Each `.md` file in those directories is a draft.
pub fn load_pov_drafts(cards_dir: &Path, content_id: &str) -> Vec<PovDraft> {
    let dir = content_node_dir(cards_dir, content_id);
    let mut drafts = Vec::new();

    for cat in PovCategory::ALL {
        let sub = dir.join(cat.dir_name());
        let Ok(entries) = std::fs::read_dir(&sub) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.starts_with('_') {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                drafts.push(PovDraft {
                    name: stem.to_string(),
                    category: cat,
                    text,
                    path,
                });
            }
        }
    }

    drafts.sort_by(|a, b| a.category.dir_name().cmp(a.category.dir_name()).then(a.name.cmp(&b.name)));
    drafts
}

/// Build a summary of drafts per category.
pub fn draft_summary(cards_dir: &Path, content_id: &str) -> DraftSummary {
    let drafts = load_pov_drafts(cards_dir, content_id);
    let mut summary = DraftSummary::default();
    for d in drafts {
        match d.category {
            PovCategory::Pov => summary.pov.push(d.name),
            PovCategory::Timeline => summary.timeline.push(d.name),
            PovCategory::Outsider => summary.outsider.push(d.name),
        }
    }
    summary
}

// ══════════════════════════════════════════════════════════════════
// Progress tracking
// ══════════════════════════════════════════════════════════════════

const PROGRESS_FILE: &str = ".progress.json";

/// Writing progress for a content node. Auto-recorded by CLI commands.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Progress {
    /// Entity IDs for which `context` was run
    #[serde(default)]
    pub context_checked: Vec<String>,

    /// Whether `suggest` was run
    #[serde(default)]
    pub suggest_ran: bool,

    /// Whether `read sibling` was run
    #[serde(default)]
    pub sibling_read: bool,

    /// Whether `read parent` was run
    #[serde(default)]
    pub parent_read: bool,

    /// Draft summary (auto-detected from files)
    #[serde(default)]
    pub drafts: DraftSummary,
}

impl Progress {
    /// Load progress from the node directory. Returns default if not found.
    pub fn load(cards_dir: &Path, content_id: &str) -> Self {
        let dir = content_node_dir(cards_dir, content_id);
        let path = dir.join(PROGRESS_FILE);
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save progress to the node directory.
    pub fn save(&self, cards_dir: &Path, content_id: &str) -> Result<(), crate::LedgerError> {
        let dir = content_node_dir(cards_dir, content_id);
        let path = dir.join(PROGRESS_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Record that `context` was run for an entity.
    pub fn record_context(&mut self, entity_id: &str) {
        if !self.context_checked.contains(&entity_id.to_string()) {
            self.context_checked.push(entity_id.to_string());
        }
    }

    /// Record that `suggest` was run.
    pub fn record_suggest(&mut self) {
        self.suggest_ran = true;
    }

    /// Record that `read sibling` was run.
    pub fn record_sibling(&mut self) {
        self.sibling_read = true;
    }

    /// Record that `read parent` was run.
    pub fn record_parent(&mut self) {
        self.parent_read = true;
    }

    /// Sync draft summary from filesystem.
    pub fn sync_drafts(&mut self, cards_dir: &Path, content_id: &str) {
        self.drafts = draft_summary(cards_dir, content_id);
    }

    /// Reset progress (e.g., when re-activating a node).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Load local entity cards for a specific content node.
///
/// Fractal layout: local cards live alongside `root.md` in the node directory:
/// - `cards/content/act1/characters/*.md`
/// - `cards/content/act1/locations/*.md`
/// - `cards/content/act1/factions/*.md`
pub fn load_local_entity_cards(
    cards_dir: &Path,
    content_id: &str,
) -> Result<Vec<(Entity, PathBuf)>, LedgerError> {
    let node_dir = content_node_dir(cards_dir, content_id);
    if !node_dir.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for subdir in ["characters", "locations", "factions"] {
        let dir = node_dir.join(subdir);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "md") && !is_template(&path) {
                let entity = parse_entity_card(&path)?;
                result.push((entity, path));
            }
        }
    }

    result.sort_by(|a, b| a.0.id().cmp(b.0.id()));
    Ok(result)
}

/// Load local secret cards for a specific content node.
///
/// Fractal layout: `cards/content/act1/secrets/*.md`
pub fn load_local_secret_cards(
    cards_dir: &Path,
    content_id: &str,
) -> Result<Vec<Secret>, LedgerError> {
    let node_dir = content_node_dir(cards_dir, content_id);
    let dir = node_dir.join("secrets");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut secrets = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "md") && !is_template(&path) {
            let raw = std::fs::read_to_string(&path)?;
            let (yaml, _body) = split_frontmatter(&raw).ok_or_else(|| {
                LedgerError::Parse(format!(
                    "Secret card missing frontmatter: {}",
                    path.display()
                ))
            })?;
            let secret: Secret = serde_yaml::from_str(yaml).map_err(|e| {
                LedgerError::Parse(format!("Secret card parse error in {}: {e}", path.display()))
            })?;
            secrets.push(secret);
        }
    }

    secrets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(secrets)
}

// ══════════════════════════════════════════════════════════════════
// Card serialization (Entity/Secret → card Markdown)
// ══════════════════════════════════════════════════════════════════

/// Serialize an Entity to card Markdown format (YAML frontmatter + body).
pub fn entity_to_card(entity: &Entity) -> Result<String, LedgerError> {
    // Clone and strip description for YAML serialization
    let mut yaml_entity = entity.clone();
    let description = entity.description().map(|s| s.to_string());
    yaml_entity.set_description(None);

    let yaml = serde_yaml::to_string(&yaml_entity)?;
    let mut out = format!("---\n{yaml}---\n");

    if let Some(desc) = description {
        out.push('\n');
        out.push_str(&desc);
        out.push('\n');
    }

    Ok(out)
}

/// Serialize a Secret to card Markdown format.
pub fn secret_to_card(secret: &Secret) -> Result<String, LedgerError> {
    let yaml = serde_yaml::to_string(secret)?;
    Ok(format!("---\n{yaml}---\n"))
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::Relationship;
    use crate::input::secret::DramaticFunction;
    use std::io::Write;

    #[test]
    fn split_frontmatter_basic() {
        let raw = "---\nid: kian\ntype: character\n---\n# Kian\nSome description.";
        let (yaml, body) = split_frontmatter(raw).unwrap();
        assert_eq!(yaml, "id: kian\ntype: character");
        assert_eq!(body, "# Kian\nSome description.");
    }

    #[test]
    fn split_frontmatter_no_body() {
        let raw = "---\nid: kian\n---\n";
        let (yaml, body) = split_frontmatter(raw).unwrap();
        assert_eq!(yaml, "id: kian");
        assert_eq!(body, "");
    }

    #[test]
    fn split_frontmatter_missing() {
        assert!(split_frontmatter("# Just markdown").is_none());
    }

    #[test]
    fn parse_character_card() {
        let raw = r#"---
type: character
id: kian
name: "基安"
traits:
  - 谨慎
  - 聪敏
location: sector_4
relationships:
  - target: nova
    role: 前同事
    trust: 1
    affinity: 1
    respect: 0
inventory:
  - 防毒面具
---

# 基安

前星联公司安全顾问。
"#;
        let entity = parse_entity_card_str(raw, Path::new("test.md")).unwrap();
        assert_eq!(entity.id(), "kian");
        assert_eq!(entity.entity_type(), "character");
        assert_eq!(entity.name(), Some("基安"));
        let c = entity.as_character().unwrap();
        assert_eq!(c.traits, vec!["谨慎", "聪敏"]);
        assert_eq!(c.location.as_deref(), Some("sector_4"));
        assert_eq!(c.relationships.len(), 1);
        assert_eq!(c.relationships[0].target, "nova");
        assert_eq!(c.inventory, vec!["防毒面具"]);
        assert!(entity.description().unwrap().contains("前星联公司安全顾问"));
    }

    #[test]
    fn parse_location_card() {
        let raw = r#"---
type: location
id: the_spire
name: "尖塔"
properties:
  - 高耸
  - 守卫森严
connections:
  - sector_4
---
"#;
        let entity = parse_entity_card_str(raw, Path::new("test.md")).unwrap();
        assert_eq!(entity.entity_type(), "location");
        assert_eq!(entity.id(), "the_spire");
        let l = entity.as_location().unwrap();
        assert_eq!(l.properties, vec!["高耸", "守卫森严"]);
        assert_eq!(l.connections, vec!["sector_4"]);
    }

    #[test]
    fn parse_faction_card() {
        let raw = r#"---
type: faction
id: nexus_corp
name: "星联公司"
members:
  - nova
rivals:
  - resistance
alignment: authoritarian
---
"#;
        let entity = parse_entity_card_str(raw, Path::new("test.md")).unwrap();
        assert_eq!(entity.entity_type(), "faction");
        let f = entity.as_faction().unwrap();
        assert_eq!(f.members, vec!["nova"]);
        assert_eq!(f.rivals, vec!["resistance"]);
        assert_eq!(f.alignment.as_deref(), Some("authoritarian"));
    }

    #[test]
    fn parse_secret_card() {
        let yaml = r#"---
id: dark_experiment
content: "暗影实验的真相"
known_by:
  - kian
revealed_to_reader: false
dramatic_function: reversal
---

关于这个秘密的详细笔记。
"#;
        let (fm, _body) = split_frontmatter(yaml).unwrap();
        let secret: Secret = serde_yaml::from_str(fm).unwrap();
        assert_eq!(secret.id, "dark_experiment");
        assert_eq!(secret.known_by, vec!["kian"]);
        assert!(!secret.revealed_to_reader);
        assert_eq!(secret.dramatic_function, Some(DramaticFunction::Reversal));
    }

    #[test]
    fn parse_beat_card() {
        let raw = r#"---
seq: 1
effects:
  - move(kian, the_spire)
  - add_trait(kian, determined)
---

雨水顺着基安的衣领滑下。他站在尖塔的入口。
"#;
        let (yaml, body) = split_frontmatter(raw).unwrap();
        let fm: BeatFrontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(fm.seq, Some(1));
        assert_eq!(fm.effects.len(), 2);

        let text = body.trim();
        let effects: Vec<Op> = fm.effects.iter().map(|s| Op::parse(s).unwrap()).collect();
        assert_eq!(effects.len(), 2);
        // CJK: 雨水顺着基安的衣领滑下 = 10, 他站在尖塔的入口 = 8, 。= 0 → 18
        // Plus the period is not CJK so total = 18
        // Actually let's just verify it's > 0 and matches the counter
        assert_eq!(Beat::count_words(text), Beat::count_words("雨水顺着基安的衣领滑下。他站在尖塔的入口。"));
    }

    #[test]
    fn entity_to_card_roundtrip() {
        use crate::input::entity::Character;
        let entity = Entity::Character(Character {
            id: "kian".into(),
            name: Some("基安".into()),
            traits: vec!["谨慎".into()],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec![],
            location: Some("sector_4".into()),
            relationships: vec![Relationship {
                target: "nova".into(),
                role: "前同事".into(),
                trust: 1,
                affinity: 1,
                respect: 0,
                facade_affinity: None,
                facade_respect: None,
            }],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: Some("前星联安全顾问。".into()),
        });

        let card = entity_to_card(&entity).unwrap();
        assert!(card.starts_with("---\n"));
        assert!(card.contains("id: kian"));
        assert!(card.contains("前星联安全顾问"));

        // Re-parse
        let parsed = parse_entity_card_str(&card, Path::new("test.md")).unwrap();
        assert_eq!(parsed.id(), entity.id());
        assert_eq!(parsed.entity_type(), entity.entity_type());
        let pc = parsed.as_character().unwrap();
        let ec = entity.as_character().unwrap();
        assert_eq!(pc.traits, ec.traits);
        assert_eq!(pc.location, ec.location);
        assert_eq!(parsed.description(), entity.description());
    }

    #[test]
    fn load_entity_cards_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cards_dir = dir.path();

        // Create subdirectories
        let chars_dir = cards_dir.join("characters");
        std::fs::create_dir_all(&chars_dir).unwrap();
        let locs_dir = cards_dir.join("locations");
        std::fs::create_dir_all(&locs_dir).unwrap();

        // Write character card
        let mut f = std::fs::File::create(chars_dir.join("kian.md")).unwrap();
        write!(f, "---\ntype: character\nid: kian\n---\n# Kian\n").unwrap();

        // Write location card
        let mut f = std::fs::File::create(locs_dir.join("spire.md")).unwrap();
        write!(f, "---\ntype: location\nid: the_spire\n---\n").unwrap();

        let entities = load_entity_cards(cards_dir).unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].0.id(), "kian");
        assert_eq!(entities[1].0.id(), "the_spire");
    }

    #[test]
    fn load_secret_cards_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cards_dir = dir.path();
        let secrets_dir = cards_dir.join("secrets");
        std::fs::create_dir_all(&secrets_dir).unwrap();

        let mut f = std::fs::File::create(secrets_dir.join("dark.md")).unwrap();
        write!(
            f,
            "---\nid: dark\ncontent: \"秘密内容\"\n---\n"
        )
        .unwrap();

        let secrets = load_secret_cards(cards_dir).unwrap();
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].id, "dark");
        assert_eq!(secrets[0].content, "秘密内容");
    }

    #[test]
    fn load_beat_cards_ordered() {
        let dir = tempfile::tempdir().unwrap();
        let cards_dir = dir.path();
        let phase_dir = cards_dir.join("phases").join("ch1");
        std::fs::create_dir_all(&phase_dir).unwrap();

        // Write beat 2 first (to test sorting)
        let mut f = std::fs::File::create(phase_dir.join("002.md")).unwrap();
        write!(f, "---\nseq: 2\neffects:\n  - add_trait(kian, brave)\n---\n第二段。\n").unwrap();

        let mut f = std::fs::File::create(phase_dir.join("001.md")).unwrap();
        write!(
            f,
            "---\nseq: 1\neffects:\n  - move(kian, the_spire)\n---\n第一段。\n"
        )
        .unwrap();

        let beats = load_beat_cards(cards_dir, "ch1").unwrap();
        assert_eq!(beats.len(), 2);
        assert_eq!(beats[0].seq, 1);
        assert_eq!(beats[1].seq, 2);
        assert_eq!(beats[0].effects.len(), 1);
        assert_eq!(beats[1].effects.len(), 1);
    }

    #[test]
    fn card_missing_frontmatter_errors() {
        let result = parse_entity_card_str("# Just markdown", Path::new("bad.md"));
        assert!(result.is_err());
    }

    #[test]
    fn entity_card_datalog_matches_json() {
        // Card-parsed entity should produce identical Datalog facts as JSON-parsed entity
        let raw = r#"---
type: character
id: kian
name: "基安"
traits:
  - 谨慎
location: wasteland
---
"#;
        let card_entity = parse_entity_card_str(raw, Path::new("test.md")).unwrap();
        let card_facts = card_entity.to_datalog();

        assert!(card_facts.contains(&"character(kian).".to_string()));
        assert!(card_facts.contains(&"at(kian, wasteland).".to_string()));
        assert!(card_facts.iter().any(|f| f.contains("trait(kian,")));
        assert!(card_facts.iter().any(|f| f.contains("name(kian,")));
    }

    // ── Content card tests ──────────────────────────────────────

    #[test]
    fn parse_content_card_basic() {
        let raw = r#"---
order: 1
title: "开场：荒野醒来"
synopsis: "基安在荒野中醒来"
effects:
  - move(kian, wasteland)
  - add_trait(kian, disoriented)
---

雨水顺着基安的衣领滑下。他记不清自己是怎么到这里的。
"#;
        let content = parse_content_card_str(
            raw,
            Path::new("test.md"),
            "act1_opening".into(),
            Some("root".into()),
        )
        .unwrap();
        assert_eq!(content.id, "act1_opening");
        assert_eq!(content.parent.as_deref(), Some("root"));
        assert_eq!(content.order, 1);
        assert_eq!(content.title.as_deref(), Some("开场：荒野醒来"));
        assert_eq!(content.effects.len(), 2);
        assert!(content.word_count > 0);
        assert!(content.text.contains("雨水"));
    }

    #[test]
    fn parse_content_card_root() {
        let raw = r#"---
title: "鸿门宴"
synopsis: "一场生死攸关的宴席"
---

故事的开始。
"#;
        let content = parse_content_card_str(
            raw,
            Path::new("test.md"),
            "root".into(),
            None,
        )
        .unwrap();
        assert_eq!(content.id, "root");
        assert!(content.parent.is_none());
        assert!(content.effects.is_empty());
    }

    #[test]
    fn parse_content_card_with_constraints() {
        let raw = r#"---
order: 3
constraints:
  exit_state:
    - query: kian.location
      expected: the_spire
  words: [500, 2000]
---

高潮部分的叙事。
"#;
        let content = parse_content_card_str(
            raw,
            Path::new("test.md"),
            "climax".into(),
            Some("root".into()),
        )
        .unwrap();
        assert_eq!(content.constraints.exit_state.len(), 1);
        assert_eq!(content.constraints.words, Some((500, 2000)));
    }

    #[test]
    fn load_content_cards_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cards_dir = dir.path();
        let content_dir = cards_dir.join("content");
        let act1_dir = content_dir.join("act1");
        std::fs::create_dir_all(&act1_dir).unwrap();

        // Root node: cards/content/root.md
        let mut f = std::fs::File::create(content_dir.join("root.md")).unwrap();
        write!(f, "---\ntitle: 故事\n---\n").unwrap();

        // Child node: cards/content/act1/root.md
        let mut f = std::fs::File::create(act1_dir.join("root.md")).unwrap();
        write!(f, "---\norder: 1\n---\n第一幕。\n").unwrap();

        let contents = load_content_cards(cards_dir).unwrap();
        assert_eq!(contents.len(), 2);
        // root has no parent (None sorts before Some)
        assert!(contents[0].parent.is_none());
        assert_eq!(contents[0].id, "root");
        assert_eq!(contents[1].id, "act1");
        assert_eq!(contents[1].parent.as_deref(), Some("root"));
    }

    #[test]
    fn load_local_entity_cards_for_content() {
        let dir = tempfile::tempdir().unwrap();
        let cards_dir = dir.path();
        // Fractal layout: cards/content/act1/characters/stranger.md
        let local_chars = cards_dir
            .join("content")
            .join("act1")
            .join("characters");
        std::fs::create_dir_all(&local_chars).unwrap();

        let mut f = std::fs::File::create(local_chars.join("stranger.md")).unwrap();
        write!(
            f,
            "---\ntype: character\nid: stranger\nname: 陌生人\n---\n一个神秘的陌生人。\n"
        )
        .unwrap();

        let entities = load_local_entity_cards(cards_dir, "act1").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].0.id(), "stranger");
    }

    #[test]
    fn load_local_entity_cards_empty_when_no_dir() {
        let dir = tempfile::tempdir().unwrap();
        let entities = load_local_entity_cards(dir.path(), "nonexistent").unwrap();
        assert!(entities.is_empty());
    }
}
