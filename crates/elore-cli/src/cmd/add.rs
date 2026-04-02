//! `elore add entity/secret/content <json>` — AI writes world state.
//!
//! All fields are optional except the primary key (id).
//! Referenced entities/secrets must already exist — hard error if not.

use std::path::Path;

use colored::Colorize;
use serde_json::{Value, json};

use ledger::input::entity;
use ledger::state::content::ContentTree;

// ══════════════════════════════════════════════════════════════════
// Entity
// ══════════════════════════════════════════════════════════════════

pub fn add_entity(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore_dir = project.join(".everlore");
    let entities_dir = everlore_dir.join("entities");
    std::fs::create_dir_all(&entities_dir)?;

    let mut v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let id = require_str(&v, "id", "entity")?;
    apply_entity_defaults(&mut v);

    // Validate references
    let existing = entity::load_entities(&entities_dir)?;
    let existing_ids: Vec<&str> = existing.iter().map(|e| e.id()).collect();

    // Check location exists
    if let Some(loc) = v.get("location").and_then(|l| l.as_str())
        && !loc.is_empty()
        && loc != "null"
        && !existing_ids.contains(&loc)
    {
        return Err(format!(
            "location '{loc}' 不存在 — 请先创建该地点实体\n  \
                 已存在的实体: {}",
            if existing_ids.is_empty() {
                "(空)".to_string()
            } else {
                existing_ids.join(", ")
            }
        )
        .into());
    }

    // Check relationship targets exist
    if let Some(rels) = v.get("relationships").and_then(|r| r.as_array()) {
        for rel in rels {
            if let Some(target) = rel.get("target").and_then(|t| t.as_str())
                && !existing_ids.contains(&target)
                && target != id.as_str()
            {
                return Err(format!(
                    "relationship target '{target}' 不存在 — 请先创建该实体\n  \
                         已存在的实体: {}",
                    existing_ids.join(", ")
                )
                .into());
            }
        }
    }

    // Upsert file
    let path = entities_dir.join(format!("{id}.json"));
    let exists = path.exists();

    let final_v = if exists {
        let old: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        merge_json(old, v)
    } else {
        v
    };

    let content = serde_json::to_string_pretty(&final_v)?;
    std::fs::write(&path, content)?;

    let action = if exists { "更新" } else { "创建" };
    let entity_type = final_v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("entity");
    println!(
        "{} {} {} → {}",
        "✓".green().bold(),
        action,
        entity_type.cyan(),
        id.bold()
    );
    println!("  {}", path.display());
    Ok(())
}

/// Batch-create entities from a JSON array, automatically resolving the
/// creation order using topological sort.
pub fn add_entities_batch(
    project: &Path,
    raw_json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let arr: Vec<Value> =
        serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败 (期望 array): {e}"))?;

    if arr.is_empty() {
        println!("{} 空 array，无实体创建", "⚠".yellow());
        return Ok(());
    }

    let ids: Vec<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    if ids.len() != arr.len() {
        return Err("所有实体必须有 'id' 字段".into());
    }

    // Build dependency edges
    let mut deps: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for v in &arr {
        let id = v["id"].as_str().unwrap().to_string();
        let mut entity_deps = Vec::new();

        if let Some(loc) = v.get("location").and_then(|l| l.as_str())
            && !loc.is_empty()
            && loc != "null"
            && ids.contains(&loc.to_string())
        {
            entity_deps.push(loc.to_string());
        }

        if let Some(rels) = v.get("relationships").and_then(|r| r.as_array()) {
            for rel in rels {
                if let Some(target) = rel.get("target").and_then(|t| t.as_str())
                    && ids.contains(&target.to_string())
                {
                    entity_deps.push(target.to_string());
                }
            }
        }

        deps.insert(id, entity_deps);
    }

    // Kahn's algorithm: topological sort
    let mut in_degree: std::collections::HashMap<&str, usize> =
        ids.iter().map(|id| (id.as_str(), 0)).collect();

    let mut reverse: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for (id, entity_deps) in &deps {
        for dep in entity_deps {
            reverse.entry(dep.as_str()).or_default().push(id.as_str());
            *in_degree.entry(id.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: std::collections::VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut sorted_ids: Vec<&str> = Vec::new();
    while let Some(id) = queue.pop_front() {
        sorted_ids.push(id);
        if let Some(dependents) = reverse.get(id) {
            for &dep in dependents {
                let count = in_degree.get_mut(dep).unwrap();
                *count -= 1;
                if *count == 0 {
                    queue.push_back(dep);
                }
            }
        }
    }

    if sorted_ids.len() != ids.len() {
        return Err("实体间存在循环依赖，无法排序".into());
    }

    let mut value_map: std::collections::HashMap<&str, &Value> = std::collections::HashMap::new();
    for v in &arr {
        let id = v["id"].as_str().unwrap();
        value_map.insert(id, v);
    }

    println!(
        "{} 批量创建 {} 个实体 (自动排序)",
        "→".cyan(),
        sorted_ids.len()
    );
    for id in &sorted_ids {
        let v = value_map[*id];
        let json = serde_json::to_string(v)?;
        add_entity(project, &json)?;
    }

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Secret
// ══════════════════════════════════════════════════════════════════

pub fn add_secret(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore_dir = project.join(".everlore");
    let entities_dir = everlore_dir.join("entities");
    std::fs::create_dir_all(&entities_dir)?;

    let mut v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let id = require_str(&v, "id", "secret")?;
    require_str(&v, "content", "secret")?;
    apply_secret_defaults(&mut v);

    // Validate known_by entities
    let existing = entity::load_entities(&entities_dir)?;
    let entity_ids: Vec<&str> = existing.iter().map(|e| e.id()).collect();

    if let Some(known_by) = v.get("known_by").and_then(|k| k.as_array()) {
        for member in known_by {
            if let Some(eid) = member.as_str()
                && !entity_ids.contains(&eid)
            {
                return Err(format!("known_by 引用了不存在的实体 '{eid}'").into());
            }
        }
    }

    // Load-upsert secrets.yaml
    let secrets_path = everlore_dir.join("secrets.yaml");
    let mut secrets_doc: Value = if secrets_path.exists() {
        let content = std::fs::read_to_string(&secrets_path)?;
        serde_yaml::from_str(&content).unwrap_or(json!({"secrets": []}))
    } else {
        json!({"secrets": []})
    };

    let arr = secrets_doc
        .get_mut("secrets")
        .and_then(|s| s.as_array_mut())
        .ok_or("secrets.yaml 格式错误: 缺少 secrets 数组")?;

    let pos = arr
        .iter()
        .position(|s| s.get("id").and_then(|i| i.as_str()) == Some(&id));

    let action = if let Some(idx) = pos {
        let old = arr[idx].clone();
        arr[idx] = merge_json(old, v);
        "更新"
    } else {
        arr.push(v);
        "创建"
    };

    let yaml = serde_yaml::to_string(&secrets_doc)?;
    std::fs::write(&secrets_path, yaml)?;

    println!("{} {} secret → {}", "✓".green().bold(), action, id.bold());
    println!("  {}", secrets_path.display());
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Content
// ══════════════════════════════════════════════════════════════════

/// Removed: add_content — create card files directly instead.
#[allow(dead_code)]
fn _add_content_removed(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let cards_dir = project.join("cards");

    let v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;
    let name = require_str(&v, "name", "content")?;

    // Determine parent: explicit or active node
    let parent_id = if let Some(p) = v.get("parent").and_then(|p| p.as_str()) {
        p.to_string()
    } else {
        let tree = ContentTree::load(&everlore);
        tree.active
            .ok_or("没有指定 parent，也没有 active 节点")?
    };

    let order = v.get("order").and_then(|o| o.as_u64()).unwrap_or(0) as u32;
    let title = v.get("title").and_then(|t| t.as_str());
    let synopsis = v.get("synopsis").and_then(|s| s.as_str());
    let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");

    // Resolve parent directory and compute order
    let parent_dir = ledger::card::content_node_dir(&cards_dir, &parent_id);
    if !parent_dir.join("root.md").exists() {
        return Err(format!("父节点 '{parent_id}' 不存在").into());
    }

    let node_dir = parent_dir.join(&name);
    let card_path = node_dir.join("root.md");
    let exists = card_path.exists();
    std::fs::create_dir_all(&node_dir)?;

    // Auto-compute order if not specified
    let order = if order == 0 {
        count_content_children(&parent_dir) as u32
    } else {
        order
    };

    // Build YAML frontmatter (no id, no parent — derived from path)
    let mut fm = serde_yaml::Mapping::new();
    fm.insert(
        serde_yaml::Value::String("order".into()),
        serde_yaml::Value::Number(serde_yaml::Number::from(order)),
    );
    if let Some(t) = title {
        fm.insert(
            serde_yaml::Value::String("title".into()),
            serde_yaml::Value::String(t.into()),
        );
    }
    if let Some(s) = synopsis {
        fm.insert(
            serde_yaml::Value::String("synopsis".into()),
            serde_yaml::Value::String(s.into()),
        );
    }

    if let Some(effects) = v.get("effects").and_then(|e| e.as_array()) {
        let yaml_effects: Vec<serde_yaml::Value> = effects
            .iter()
            .filter_map(|e| e.as_str().map(|s| serde_yaml::Value::String(s.into())))
            .collect();
        if !yaml_effects.is_empty() {
            fm.insert(
                serde_yaml::Value::String("effects".into()),
                serde_yaml::Value::Sequence(yaml_effects),
            );
        }
    }

    if let Some(slots) = v.get("inherited_slots").and_then(|e| e.as_array()) {
        let yaml_slots: Vec<serde_yaml::Value> = slots
            .iter()
            .filter_map(|e| e.as_str().map(|s| serde_yaml::Value::String(s.into())))
            .collect();
        if !yaml_slots.is_empty() {
            fm.insert(
                serde_yaml::Value::String("inherited_slots".into()),
                serde_yaml::Value::Sequence(yaml_slots),
            );
        }
    }

    if let Some(constraints) = v.get("constraints") {
        let yaml_constraints: serde_yaml::Value =
            serde_json::from_value(constraints.clone()).unwrap_or(serde_yaml::Value::Null);
        if !yaml_constraints.is_null() {
            fm.insert(
                serde_yaml::Value::String("constraints".into()),
                yaml_constraints,
            );
        }
    }

    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(fm))?;
    let card_content = if text.is_empty() {
        format!("---\n{yaml}---\n")
    } else {
        format!("---\n{yaml}---\n\n{text}\n")
    };

    std::fs::write(&card_path, card_content)?;

    // Rebuild content tree
    let contents = ledger::card::load_content_cards(&cards_dir)?;
    let mut tree = ContentTree::load(&everlore);
    let mut orders = std::collections::BTreeMap::new();
    for c in &contents {
        orders.insert(c.id.clone(), c.order);
        tree.register(c);
    }
    tree.sort_children(&orders);
    tree.save(&everlore)?;

    // Derive content id from path
    let content_id = if parent_id == "root" {
        name.clone()
    } else {
        format!("{parent_id}/{name}")
    };

    let action = if exists { "更新" } else { "创建" };
    let status = tree
        .nodes
        .get(&content_id)
        .map(|e| format!("{:?}", e.status))
        .unwrap_or_else(|| "?".into());

    println!(
        "{} {} content → {} ({})",
        "✓".green().bold(),
        action,
        content_id.cyan().bold(),
        status
    );
    println!("  parent: {parent_id}");
    println!("  {}", card_path.display());
    Ok(())
}

#[allow(dead_code)]
fn count_content_children(parent_dir: &Path) -> usize {
    let reserved = ["characters", "locations", "factions", "secrets"];
    std::fs::read_dir(parent_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().is_dir()
                        && !reserved.contains(&e.file_name().to_string_lossy().as_ref())
                })
                .count()
        })
        .unwrap_or(0)
        + 1 // 1-indexed
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

fn apply_entity_defaults(v: &mut Value) {
    set_default(v, "type", json!("character"));
    set_default(v, "traits", json!([]));
    set_default(v, "beliefs", json!([]));
    set_default(v, "desires", json!([]));
    set_default(v, "intentions", json!([]));
    set_default(v, "relationships", json!([]));
    set_default(v, "inventory", json!([]));
    set_default(v, "tags", json!(["active"]));
    match v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("character")
    {
        "location" => {
            set_default(v, "properties", json!([]));
            set_default(v, "connections", json!([]));
        }
        "faction" => {
            set_default(v, "members", json!([]));
            set_default(v, "rivals", json!([]));
        }
        _ => {}
    }
}

fn apply_secret_defaults(v: &mut Value) {
    set_default(v, "known_by", json!([]));
    set_default(v, "revealed_to_reader", json!(false));
    set_default(v, "dramatic_function", json!("suspense"));
}

fn set_default(v: &mut Value, key: &str, default: Value) {
    if v.get(key).is_none()
        && let Value::Object(map) = v
    {
        map.insert(key.to_string(), default);
    }
}

fn merge_json(mut base: Value, patch: Value) -> Value {
    if let (Value::Object(base_map), Value::Object(patch_map)) = (&mut base, patch) {
        for (k, v) in patch_map {
            base_map.insert(k, v);
        }
    }
    base
}

fn require_str(v: &Value, field: &str, ctx: &str) -> Result<String, Box<dyn std::error::Error>> {
    v.get(field)
        .and_then(|f| f.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{ctx}: 缺少必填字段 '{field}'").into())
}
