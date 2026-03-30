//! `elore add entity/drama/secret <json>` — AI writes world state without touching files.
//!
//! All fields are optional except the primary key (id / chapter).
//! Referenced entities/secrets must already exist — hard error if not.

use std::path::Path;

use colored::Colorize;
use serde_json::{Value, json};

use ledger::effect::beat::Beat;
use ledger::effect::history::History;
use ledger::input::{entity, secret};
use ledger::state::constraint::check_assertions;
use ledger::state::phase::{BeatPlan, ConstraintSource, Phase, StateAssertion};
use ledger::state::phase_manager::ProjectState;
use ledger::state::snapshot::Snapshot;

// ══════════════════════════════════════════════════════════════════
// Entry points
// ══════════════════════════════════════════════════════════════════

pub fn add_entity(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore_dir = project.join(".everlore");
    let entities_dir = everlore_dir.join("entities");
    std::fs::create_dir_all(&entities_dir)?;

    // Parse and apply defaults
    let mut v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let id = require_str(&v, "id", "entity")?;
    apply_entity_defaults(&mut v);

    // Validate references
    let existing = entity::load_entities(&entities_dir)?;
    let existing_secrets = secret::load_secrets(&everlore_dir)?;
    let existing_ids: Vec<&str> = existing.iter().map(|e| e.id()).collect();
    let _secret_ids: Vec<&str> = existing_secrets.iter().map(|s| s.id.as_str()).collect();

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

    // If updating, merge with existing file
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
///
/// This removes the need to manually order entities: location before
/// character, etc. Just pass them all at once.
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

    // Collect all IDs from the batch
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

    // Build dependency edges: id → depends_on ids
    // Dependencies: location field, relationship targets
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

    // Build reverse: for each dep → list of nodes that need it first
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

    // Build id → value map
    let mut value_map: std::collections::HashMap<&str, &Value> = std::collections::HashMap::new();
    for v in &arr {
        let id = v["id"].as_str().unwrap();
        value_map.insert(id, v);
    }

    // Create in topological order
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

    // Upsert by id
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
// Default fillers
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
    // type-specific defaults
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

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

/// Set a field only if it's missing.
fn set_default(v: &mut Value, key: &str, default: Value) {
    if v.get(key).is_none()
        && let Value::Object(map) = v
    {
        map.insert(key.to_string(), default);
    }
}

/// Shallow merge: `patch` fields overwrite `base`, arrays not merged.
fn merge_json(mut base: Value, patch: Value) -> Value {
    if let (Value::Object(base_map), Value::Object(patch_map)) = (&mut base, patch) {
        for (k, v) in patch_map {
            base_map.insert(k, v);
        }
    }
    base
}

/// Require a string field, return it.
fn require_str(v: &Value, field: &str, ctx: &str) -> Result<String, Box<dyn std::error::Error>> {
    v.get(field)
        .and_then(|f| f.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{ctx}: 缺少必填字段 '{field}'").into())
}

// ══════════════════════════════════════════════════════════════════
// v3: Phase / Beat / Note
// ══════════════════════════════════════════════════════════════════

pub fn add_phase(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases_dir = everlore.join("phases");
    let entities_dir = everlore.join("entities");
    std::fs::create_dir_all(&phases_dir)?;

    let v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let id = require_str(&v, "id", "phase")?;

    // Build Phase struct — serde handles defaults
    let mut phase: Phase =
        serde_json::from_value(v.clone()).map_err(|e| format!("Phase 解析失败: {e}"))?;
    let mut derivation_notes = Vec::new();
    if phase.synopsis.is_some() && !phase.has_any_constraints() {
        let entities = entity::load_entities(&entities_dir)?;
        derivation_notes = derive_phase_constraints_from_synopsis(&mut phase, &entities);
        phase.constraint_source = Some(ConstraintSource::SynopsisDerived);
    } else if phase.has_any_constraints() && phase.constraint_source.is_none() {
        phase.constraint_source = Some(ConstraintSource::Manual);
    }

    // Save definition
    phase.save(&phases_dir)?;

    // Register in state.json
    let mut state = ProjectState::load(&everlore);
    state.register_phase(&phase);
    state.save(&everlore)?;

    let status = &state.phases[&id].status;
    println!(
        "{} 创建 phase → {} ({:?})",
        "✓".green().bold(),
        id.cyan().bold(),
        status
    );
    if let Some(syn) = &phase.synopsis {
        println!("  {}", syn.dimmed());
    }
    println!("  definition: {:?}", phase.definition_status());
    for note in derivation_notes {
        println!("  {} {}", "→".dimmed(), note);
    }
    Ok(())
}

fn derive_phase_constraints_from_synopsis(
    phase: &mut Phase,
    entities: &[ledger::Entity],
) -> Vec<String> {
    let mut notes = Vec::new();
    let synopsis = phase.synopsis.clone().unwrap_or_default();

    let mentioned_entities: Vec<&ledger::Entity> = entities
        .iter()
        .filter(|entity| {
            synopsis.contains(entity.id())
                || entity
                    .name()
                    .is_some_and(|name| synopsis.contains(name))
        })
        .collect();

    if !mentioned_entities.is_empty() {
        phase.constraints.ledger.invariants = mentioned_entities
            .iter()
            .map(|entity| StateAssertion {
                query: format!("entity_alive({})", entity.id()),
                expected: "true".into(),
            })
            .collect();
        notes.push(format!(
            "从 synopsis 匹配实体，自动生成 {} 条 L1 invariants",
            phase.constraints.ledger.invariants.len()
        ));
    } else {
        notes.push("未从 synopsis 匹配到实体，L1 仍需人工补充".into());
    }

    phase.constraints.resolver.min_effects =
        Some(phase.constraints.resolver.min_effects.unwrap_or(1));
    notes.push("为 L2 设置默认 min_effects = 1".into());

    if phase.constraints.executor.words.is_none() {
        phase.constraints.executor.words = Some((800, 4000));
        notes.push("为 L3 设置默认 words = 800-4000".into());
    }
    if phase.constraints.executor.writing_plan.is_empty() {
        phase.constraints.executor.writing_plan.push(BeatPlan {
            label: "phase_arc".into(),
            target_words: Some(1200),
            effects: vec![],
            guidance: Some(synopsis),
        });
        notes.push("为 L3 生成单步 writing_plan 占位".into());
    }

    if phase.constraints.evaluator.min_avg_score.is_none() {
        phase.constraints.evaluator.min_avg_score = Some(3.0);
    }
    if phase.constraints.evaluator.max_boring_beats.is_none() {
        phase.constraints.evaluator.max_boring_beats = Some(0);
    }
    notes.push("为 L4 设置默认 min_avg_score = 3.0, max_boring_beats = 0".into());

    notes
}

pub fn add_beat(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let beats_dir = everlore.join("beats");
    let phases_dir = everlore.join("phases");
    let entities_dir = everlore.join("entities");
    std::fs::create_dir_all(&beats_dir)?;

    let state = ProjectState::load(&everlore);
    let phase_id = state
        .active_phase()
        .ok_or("没有活跃的 phase — 请先 `elore checkout <phase_id>`")?
        .to_string();

    let phase = Phase::load(&phases_dir, &phase_id)?;

    let v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let text = require_str(&v, "text", "beat")?;
    let word_count = Beat::count_words(&text);

    // Parse effects
    let effects: Vec<ledger::Op> = if let Some(effects_val) = v.get("effects") {
        if let Some(arr) = effects_val.as_array() {
            arr.iter()
                .filter_map(|e| {
                    if let Some(s) = e.as_str() {
                        ledger::Op::parse(s).ok()
                    } else {
                        serde_json::from_value(e.clone()).ok()
                    }
                })
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let created_by = v
        .get("created_by")
        .and_then(|c| c.as_str())
        .unwrap_or("ai")
        .to_string();

    // ── Step 1: Check L1 invariants BEFORE writing anything to disk ──
    // Snapshot is built from genesis + existing effects; the new beat's
    // effects don't need to be committed first for invariant checking.
    let snap = Snapshot::build(&phase_id, &entities_dir, &everlore).map_err(|e| {
        format!(
            "Snapshot 构建失败: {e}\n\
                 提示: 运行 `elore read snapshot {phase_id} --format json` 检查实体列表"
        )
    })?;

    let (inv_ok, inv_failures) = check_assertions(&snap, &phase.constraints.ledger.invariants);
    if !inv_ok {
        println!("{} L1 invariant 违反 — beat 已拒绝:", "✗".red().bold());
        for f in &inv_failures {
            println!("  {}", f.red());
        }
        println!(
            "  提示: snapshot 中的实体 IDs: {:?}",
            snap.entities.iter().map(|e| e.id()).collect::<Vec<_>>()
        );
        return Err("L1 invariant violation".into());
    }

    // ── Step 2: Allocate seq and save ──
    let seq = Beat::next_seq(&beats_dir, &phase_id);
    let beat = Beat {
        phase: phase_id.clone(),
        seq,
        revises: None,
        revision: 0,
        text,
        effects: effects.clone(),
        word_count,
        created_by,
        created_at: String::new(),
        revision_reason: None,
    };

    // If save fails, nothing was written — no rollback needed.
    beat.save(&beats_dir)?;

    // ── Step 3: Append effects to history.jsonl so Snapshot::build will replay them ──
    // This is the bridge that makes L1 invariant checks reflect the actual
    // post-beat world state rather than just the genesis state.
    let history_entries = beat.as_history_entries();
    if !history_entries.is_empty() {
        History::append(&everlore, &history_entries)?;
    }

    // ── Step 4: Update progress counters ──
    let all_beats = Beat::load_phase(&beats_dir, &phase_id);
    let total_words = Beat::total_words(&all_beats);
    let total_effects: u32 = all_beats.iter().map(|b| b.effects.len() as u32).sum();

    let mut state = ProjectState::load(&everlore);
    state.update_progress(all_beats.len() as u32, total_words, total_effects);
    state.save(&everlore)?;

    println!(
        "{} beat #{} ({} 字, {} effects)",
        "✓".green().bold(),
        seq,
        word_count,
        effects.len()
    );
    println!(
        "  phase: {} — 累计: {} 字, {} beats",
        phase_id.cyan(),
        total_words,
        all_beats.len()
    );
    Ok(())
}

pub fn add_note(project: &Path, raw_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let annotations_dir = everlore.join("annotations");

    let v: Value = serde_json::from_str(raw_json).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let state = ProjectState::load(&everlore);
    let phase_id = state.active_phase().ok_or("没有活跃的 phase")?.to_string();

    let ann: evaluator::annotation::Annotation =
        serde_json::from_value(v).map_err(|e| format!("Annotation 解析失败: {e}"))?;

    evaluator::annotation::add_annotation(&annotations_dir, &phase_id, &ann)?;

    println!(
        "{} 标注 beat #{} — score: {}, tags: {:?}",
        "✓".green().bold(),
        ann.beat,
        ann.score,
        ann.tags
    );
    Ok(())
}
