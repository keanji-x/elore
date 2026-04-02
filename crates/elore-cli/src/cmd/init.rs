//! `elore init` / `elore new` — project setup and entity scaffolding.

use colored::Colorize;
use std::path::Path;

// ── Card templates ──────────────────────────────────────────────

const CHARACTER_TEMPLATE: &str = r#"---
type: character
id: example_char
name: "角色名"
traits: [勇敢, 谨慎]
beliefs: [正义终将胜利]
desires: [找到真相]
intentions: []
intent_targets: []               # 结构化意图: [{action: 刺杀, target: enemy_id}]
desire_tags: []                  # 标准化标签: [protect_ally, seek_power]
location: example_location
relationships:
  - target: another_char
    role: 同伴
    trust: 2                     # -3~+3: 信任 → would_confide, betrayal
    affinity: 2                  # -3~+3: 亲疏 → would_sacrifice, personal_bond
    respect: 1                   # -3~+3: 敬畏 → would_obey, pressure
inventory: [长剑]
goals:
  - id: example_goal
    want: 达成某个目标
    problem: 面临某个障碍
    status: active
tags:
  - active
---

# 角色名

角色的背景描述。

<!-- 删除此模板文件后运行 elore build -->
"#;

const LOCATION_TEMPLATE: &str = r#"---
type: location
id: example_location
name: "地点名"
properties: [险峻, 灵气充沛]
connections: [another_location]
tags:
  - active
---

# 地点名

地点的描述。

<!-- 删除此模板文件后运行 elore build -->
"#;

const FACTION_TEMPLATE: &str = r#"---
type: faction
id: example_faction
name: "势力名"
leader: example_char
members: [example_char]
rivals: [enemy_faction]
tags:
  - active
---

# 势力名

势力的描述。

<!-- 删除此模板文件后运行 elore build -->
"#;

const SECRET_TEMPLATE: &str = r#"---
id: example_secret
content: 秘密的具体内容
known_by: []
revealed_to_reader: false
dramatic_function: suspense
---

秘密的补充说明。dramatic_function 可选: reversal, suspense, foreshadowing, misdirection

<!-- 删除此模板文件后运行 elore build -->
"#;

const CONTENT_TEMPLATE: &str = r#"---
title: "故事标题"
synopsis: "故事的核心概述"
# main_role: protagonist_id
# effects:
#   - move(char_id, location_id)
# constraints:
#   ledger:
#     exit_state:
#       - query: char_id.location
#         expected: destination
#   executor:
#     words: [500, 3000]
# style:
#   - 克制，少用形容词
#   - 对话推动情节
# style_override: false
---

根节点的叙事文本。这是整个故事的起点。

子节点通过子目录创建：
  cards/content/act1/root.md    ← 子节点
  cards/content/act1/scene1/root.md ← 孙节点

<!-- 删除此模板后运行 elore build -->
"#;

/// Write a template file only if it doesn't already exist.
fn write_template(path: &Path, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// Initialize a new EverLore project.
pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let entities_cache = everlore.join("entities");

    // cards/ — source of truth
    let cards = project.join("cards");
    let cards_characters = cards.join("characters");
    let cards_locations = cards.join("locations");
    let cards_factions = cards.join("factions");
    let cards_secrets = cards.join("secrets");
    let cards_content = cards.join("content");

    // Create cards/ directories
    std::fs::create_dir_all(&cards_characters)?;
    std::fs::create_dir_all(&cards_locations)?;
    std::fs::create_dir_all(&cards_factions)?;
    std::fs::create_dir_all(&cards_secrets)?;
    std::fs::create_dir_all(&cards_content)?;

    // Create .everlore/ directories (build artifacts)
    std::fs::create_dir_all(&entities_cache)?;

    // Template files — show the card format
    write_template(&cards_characters.join("_template.md"), CHARACTER_TEMPLATE)?;
    write_template(&cards_locations.join("_template.md"), LOCATION_TEMPLATE)?;
    write_template(&cards_factions.join("_template.md"), FACTION_TEMPLATE)?;
    write_template(&cards_secrets.join("_template.md"), SECRET_TEMPLATE)?;
    write_template(&cards_content.join("root.md"), CONTENT_TEMPLATE)?;

    // .everlore/.gitignore — all build artifacts
    let gitignore_path = everlore.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(
            &gitignore_path,
            "# Build artifacts — regenerate with `elore build`\n*\n!.gitignore\n",
        )?;
    }

    println!("{}", "✓ EverLore 项目已初始化".green().bold());
    println!("  {} (source of truth)", cards.display());
    println!("    characters/ locations/ factions/ secrets/");
    println!("    content/          ← 内容树 (目录即结构)");
    println!("  {} (build artifacts)", everlore.display());
    println!();
    println!("  下一步:");
    println!("    1. 在 cards/ 下创建实体卡片");
    println!("    2. 编辑 {} 填写故事根节点", "cards/content/root.md".bold());
    println!("    3. 运行 {} 编译", "elore build".bold());
    println!("    4. 运行 {} 激活根节点", "elore activate root".bold());
    println!("    5. 在 cards/content/ 下创建子目录作为子节点");
    println!("    6. 运行 {} 查看树", "elore tree".bold());
    Ok(())
}

/// Create a new entity card scaffold.
pub fn new_entity(
    project: &Path,
    entity_type: &str,
    id: &str,
    name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let subdir = match entity_type {
        "character" => "characters",
        "location" => "locations",
        "faction" => "factions",
        _ => {
            return Err(
                format!("未知类型: {entity_type} (可选: character, location, faction)").into(),
            );
        }
    };

    let cards_dir = project.join("cards").join(subdir);
    std::fs::create_dir_all(&cards_dir)?;

    let path = cards_dir.join(format!("{id}.md"));
    if path.exists() {
        return Err(format!("实体 {id} 已存在: {}", path.display()).into());
    }

    let display_name = name.unwrap_or(id);
    let content = match entity_type {
        "character" => format!(
            "---\ntype: character\nid: {id}\nname: \"{display_name}\"\ntraits: []\nbeliefs: []\ndesires: []\nintentions: []\nrelationships: []\ninventory: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        "location" => format!(
            "---\ntype: location\nid: {id}\nname: \"{display_name}\"\nproperties: []\nconnections: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        "faction" => format!(
            "---\ntype: faction\nid: {id}\nname: \"{display_name}\"\nmembers: []\nrivals: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        _ => unreachable!(),
    };

    std::fs::write(&path, content)?;

    println!(
        "{} {} {}",
        "✓".green().bold(),
        entity_type.cyan(),
        id.bold()
    );
    println!("  → {}", path.display());
    Ok(())
}
