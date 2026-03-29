//! Desire Engine: character-driven goal graph.
//!
//! Each character has a recursive goal tree: `want → problem → solution → children`.
//! Cross-character references (`conflicts_with`, `blocked_by`) create the dramatic
//! conflict graph that drives narrative causality.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// Status of a goal node in the desire tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalStatus {
    Background,
    Active,
    Blocked,
    Resolved,
    Failed,
}

impl GoalStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Active => "active",
            Self::Blocked => "blocked",
            Self::Resolved => "resolved",
            Self::Failed => "failed",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Background => "◌",
            Self::Active => "●",
            Self::Blocked => "⊘",
            Self::Resolved => "✓",
            Self::Failed => "✗",
        }
    }
}

/// A single goal node in a character's desire tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub want: String,

    #[serde(default)]
    pub problem: Option<String>,
    #[serde(default)]
    pub solution: Option<String>,
    #[serde(default = "default_active")]
    pub status: GoalStatus,

    /// Cross-character blockers: `"owner/goal_id"`.
    #[serde(default)]
    pub blocked_by: Vec<String>,
    /// Cross-character conflicts: `"owner/goal_id"`.
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    /// Side effects when this goal resolves.
    #[serde(default)]
    pub side_effects: Vec<String>,

    /// Recursive sub-goals.
    #[serde(default)]
    pub children: Vec<Goal>,
}

fn default_active() -> GoalStatus {
    GoalStatus::Active
}

/// A character entity with its goal tree, loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalEntity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub goals: Vec<Goal>,
}

// ══════════════════════════════════════════════════════════════════
// Loading
// ══════════════════════════════════════════════════════════════════

/// Load all YAML entity files that have goal trees.
pub fn load_goal_entities(dir: &Path) -> Result<Vec<GoalEntity>, LedgerError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let is_yaml = path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml");
        if !is_yaml {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        // Skip files that don't parse as GoalEntity (e.g. secrets.yaml)
        let entity: GoalEntity = match serde_yaml::from_str(&content) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entity.goals.is_empty() {
            result.push(entity);
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
// Traversal & queries
// ══════════════════════════════════════════════════════════════════

/// A flattened view of a goal node with owner context.
#[derive(Debug, Clone)]
pub struct FlatGoal {
    pub owner: String,
    pub depth: usize,
    pub goal: Goal,
}

/// Flatten all goal trees across all characters.
pub fn flatten_all(entities: &[GoalEntity]) -> Vec<FlatGoal> {
    let mut result = Vec::new();
    for entity in entities {
        for goal in &entity.goals {
            flatten_recursive(&entity.id, goal, 0, &mut result);
        }
    }
    result
}

fn flatten_recursive(owner: &str, goal: &Goal, depth: usize, out: &mut Vec<FlatGoal>) {
    out.push(FlatGoal {
        owner: owner.to_string(),
        depth,
        goal: Goal {
            children: Vec::new(),
            ..goal.clone()
        },
    });
    for child in &goal.children {
        flatten_recursive(owner, child, depth + 1, out);
    }
}

/// Find all goals with `solution = null` and `status = active` (narrative suspense).
pub fn find_suspense(entities: &[GoalEntity]) -> Vec<FlatGoal> {
    flatten_all(entities)
        .into_iter()
        .filter(|fg| fg.goal.solution.is_none() && fg.goal.status == GoalStatus::Active)
        .collect()
}

/// Find blocked goals whose blockers have resolved or failed (newly unblocked).
pub fn find_unblocked(entities: &[GoalEntity]) -> Vec<(FlatGoal, String)> {
    let flat = flatten_all(entities);

    let status_map: HashMap<String, &GoalStatus> = flat
        .iter()
        .map(|fg| (format!("{}/{}", fg.owner, fg.goal.id), &fg.goal.status))
        .collect();

    let mut result = Vec::new();
    for fg in &flat {
        if fg.goal.status != GoalStatus::Blocked {
            continue;
        }
        for blocker in &fg.goal.blocked_by {
            if let Some(status) = status_map.get(blocker.as_str())
                && (**status == GoalStatus::Failed || **status == GoalStatus::Resolved)
            {
                result.push((fg.clone(), blocker.clone()));
            }
        }
    }
    result
}

/// Find all active conflicts (both sides have status = active).
pub fn find_active_conflicts(entities: &[GoalEntity]) -> Vec<(FlatGoal, FlatGoal)> {
    let flat = flatten_all(entities);

    let lookup: HashMap<String, &FlatGoal> = flat
        .iter()
        .map(|fg| (format!("{}/{}", fg.owner, fg.goal.id), fg))
        .collect();

    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for fg in &flat {
        if fg.goal.status != GoalStatus::Active {
            continue;
        }
        let my_key = format!("{}/{}", fg.owner, fg.goal.id);
        for conflict_ref in &fg.goal.conflicts_with {
            if let Some(other) = lookup.get(conflict_ref.as_str())
                && other.goal.status == GoalStatus::Active
            {
                let pair = if my_key < *conflict_ref {
                    format!("{my_key}|{conflict_ref}")
                } else {
                    format!("{conflict_ref}|{my_key}")
                };
                if seen.insert(pair) {
                    result.push((fg.clone(), (*other).clone()));
                }
            }
        }
    }
    result
}

// ══════════════════════════════════════════════════════════════════
// Datalog translation
// ══════════════════════════════════════════════════════════════════

/// Translate all goal entities into Datalog facts.
pub fn goals_to_datalog(entities: &[GoalEntity]) -> Vec<String> {
    let mut facts = vec!["% === Goal Graph Facts ===".to_string()];
    for entity in entities {
        for goal in &entity.goals {
            emit_goal_facts(&entity.id, goal, None, &mut facts);
        }
    }
    facts
}

fn emit_goal_facts(owner: &str, goal: &Goal, parent: Option<&str>, out: &mut Vec<String>) {
    let qid = quote_id(&goal.id);
    let want_esc = goal.want.replace('"', "\\\"");

    out.push(format!("want({owner}, {qid}, \"{want_esc}\")."));
    out.push(format!(
        "goal_status({owner}, {qid}, {}).",
        goal.status.label()
    ));

    if goal.solution.is_some() {
        out.push(format!("has_solution({owner}, {qid})."));
    }
    if let Some(problem) = &goal.problem {
        let esc = problem.replace('"', "\\\"");
        out.push(format!("goal_problem({owner}, {qid}, \"{esc}\")."));
    }
    if let Some(parent_id) = parent {
        out.push(format!("child_of({qid}, {}).", quote_id(parent_id)));
    }

    for ref_path in &goal.blocked_by {
        if let Some((ref_owner, ref_goal)) = ref_path.split_once('/') {
            out.push(format!(
                "blocks({ref_owner}, {}, {owner}, {qid}).",
                quote_id(ref_goal)
            ));
        }
    }
    for ref_path in &goal.conflicts_with {
        if let Some((ref_owner, ref_goal)) = ref_path.split_once('/') {
            out.push(format!(
                "conflicts({owner}, {qid}, {ref_owner}, {}).",
                quote_id(ref_goal)
            ));
        }
    }

    for child in &goal.children {
        emit_goal_facts(owner, child, Some(&goal.id), out);
    }
}

/// Built-in Datalog rules for goal graph reasoning.
pub fn goal_rules() -> &'static str {
    r#"% === Goal Reasoning Rules ===

% Suspense: active goal with no solution
suspense(?Owner, ?Goal) :-
  goal_status(?Owner, ?Goal, active),
  ~has_solution(?Owner, ?Goal).

% Active conflict: both sides active
active_conflict(?OA, ?GA, ?OB, ?GB) :-
  conflicts(?OA, ?GA, ?OB, ?GB),
  goal_status(?OA, ?GA, active),
  goal_status(?OB, ?GB, active).

% Unblocked: a blocked goal whose blocker has failed or resolved
unblocked(?Owner, ?Goal) :-
  goal_status(?Owner, ?Goal, blocked),
  blocks(?BOwner, ?BGoal, ?Owner, ?Goal),
  goal_status(?BOwner, ?BGoal, failed).

unblocked(?Owner, ?Goal) :-
  goal_status(?Owner, ?Goal, blocked),
  blocks(?BOwner, ?BGoal, ?Owner, ?Goal),
  goal_status(?BOwner, ?BGoal, resolved).

% Cascade: if a blocker resolves, what might unblock
would_unblock(?BOwner, ?BGoal, ?Owner, ?Goal) :-
  blocks(?BOwner, ?BGoal, ?Owner, ?Goal),
  goal_status(?Owner, ?Goal, blocked).
"#
}

// ══════════════════════════════════════════════════════════════════
// Display
// ══════════════════════════════════════════════════════════════════

/// Render a character's goal tree as formatted text.
pub fn render_goal_tree(entity: &GoalEntity) -> String {
    let mut out = String::new();
    let name = entity.name.as_deref().unwrap_or(&entity.id);
    out.push_str(&format!("{} ({}) 目标树:\n\n", entity.id, name));
    for goal in &entity.goals {
        render_recursive(goal, 0, &mut out);
    }
    out
}

fn render_recursive(goal: &Goal, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth + 1);
    let icon = goal.status.icon();
    let status = goal.status.label();

    out.push_str(&format!(
        "{indent}{icon} {}: {} [{status}]\n",
        goal.id, goal.want
    ));
    if let Some(problem) = &goal.problem {
        out.push_str(&format!("{indent}  问题: {problem}\n"));
    }
    match &goal.solution {
        Some(s) => out.push_str(&format!("{indent}  解法: {s}\n")),
        None => out.push_str(&format!("{indent}  解法: ???\n")),
    }
    for b in &goal.blocked_by {
        out.push_str(&format!("{indent}  ⊘ blocked_by: {b}\n"));
    }
    for c in &goal.conflicts_with {
        out.push_str(&format!("{indent}  ⚔ conflicts: {c}\n"));
    }
    for child in &goal.children {
        render_recursive(child, depth + 1, out);
    }
}

/// Render the situation summary (suspense, conflicts, unblocked).
pub fn render_plan(entities: &[GoalEntity]) -> String {
    let mut out = String::from("═══ 当前态势 ═══\n\n");

    let suspense = find_suspense(entities);
    if !suspense.is_empty() {
        out.push_str("悬念（active, solution=null）:\n");
        for fg in &suspense {
            let problem = fg.goal.problem.as_deref().unwrap_or("—");
            out.push_str(&format!("  ● {}/{} — {}\n", fg.owner, fg.goal.id, problem));
        }
        out.push('\n');
    }

    let unblocked = find_unblocked(entities);
    if !unblocked.is_empty() {
        out.push_str("刚解锁:\n");
        for (fg, blocker) in &unblocked {
            out.push_str(&format!(
                "  ★ {}/{} — blocker {} 已失效\n",
                fg.owner, fg.goal.id, blocker
            ));
        }
        out.push('\n');
    }

    let conflicts = find_active_conflicts(entities);
    if !conflicts.is_empty() {
        out.push_str("活跃冲突:\n");
        for (a, b) in &conflicts {
            out.push_str(&format!(
                "  ⚔ {}/{} ↔ {}/{}\n",
                a.owner, a.goal.id, b.owner, b.goal.id
            ));
        }
        out.push('\n');
    }

    if suspense.is_empty() && unblocked.is_empty() && conflicts.is_empty() {
        out.push_str("  (无活跃态势 — 需要引入新的冲突或目标)\n");
    }
    out
}

// ── Helpers ──────────────────────────────────────────────────────

fn quote_id(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('"', "\\\""))
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_goal_entities() -> Vec<GoalEntity> {
        vec![
            GoalEntity {
                entity_type: "character".into(),
                id: "kian".into(),
                name: Some("基安".into()),
                goals: vec![Goal {
                    id: "survive_drought".into(),
                    want: "找到干净水源生存下去".into(),
                    problem: Some("荒野污染严重".into()),
                    solution: None,
                    status: GoalStatus::Active,
                    blocked_by: vec![],
                    conflicts_with: vec!["nova/harvest_intruder".to_string()],
                    side_effects: vec![],
                    children: vec![],
                }],
            },
            GoalEntity {
                entity_type: "character".into(),
                id: "nova".into(),
                name: Some("诺娃".into()),
                goals: vec![Goal {
                    id: "harvest_intruder".into(),
                    want: "猎杀入侵者".into(),
                    problem: None,
                    solution: None,
                    status: GoalStatus::Active,
                    blocked_by: vec![],
                    conflicts_with: vec!["kian/survive_drought".to_string()],
                    side_effects: vec![],
                    children: vec![],
                }],
            },
        ]
    }

    #[test]
    fn find_suspense_active_no_solution() {
        let entities = make_goal_entities();
        let suspense = find_suspense(&entities);
        assert_eq!(suspense.len(), 2);
    }

    #[test]
    fn find_active_conflicts_bidirectional() {
        let entities = make_goal_entities();
        let conflicts = find_active_conflicts(&entities);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn goals_to_datalog_generates_facts() {
        let entities = make_goal_entities();
        let facts = goals_to_datalog(&entities);
        assert!(facts.iter().any(|f| f.contains("want(kian,")));
        assert!(facts.iter().any(|f| f.contains("conflicts(kian,")));
    }
}
