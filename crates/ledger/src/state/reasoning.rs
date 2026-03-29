//! Datalog reasoning via Nemo — the logical inference engine.
//!
//! Translates the world state into Datalog facts and rules, runs Nemo,
//! and extracts the derived results. Zero LLM tokens consumed.
//!
//! Key derived predicates:
//! - `can_meet(A, B)` — characters at the same location
//! - `enemy(A, B)` — members of rival factions
//! - `danger(A, B)` — enemies that can meet
//! - `suspense(Owner, Goal)` — active goals without solutions
//! - `active_conflict(OA, GA, OB, GB)` — both sides of a goal conflict are active
//! - `dramatic_irony(Secret, Uninformed)` — reader knows, character doesn't

use std::collections::HashMap;

use nemo_physical::datavalues::DataValue;

use crate::LedgerError;

// ══════════════════════════════════════════════════════════════════
// Result type
// ══════════════════════════════════════════════════════════════════

/// Result from a reasoning session.
#[derive(Debug, Clone, Default)]
pub struct ReasoningResult {
    /// Derived facts keyed by predicate name.
    pub predicates: HashMap<String, Vec<Vec<String>>>,
    /// Total number of derived facts.
    pub total_facts: usize,
}

impl ReasoningResult {
    /// Get results for a specific predicate.
    pub fn get(&self, predicate: &str) -> Option<&Vec<Vec<String>>> {
        self.predicates.get(predicate)
    }

    /// True if the result contains facts for a predicate.
    pub fn has(&self, predicate: &str) -> bool {
        self.predicates
            .get(predicate)
            .is_some_and(|v| !v.is_empty())
    }

    /// Check if a specific tuple exists for a predicate.
    pub fn contains(&self, predicate: &str, args: &[&str]) -> bool {
        self.predicates.get(predicate).is_some_and(|rows| {
            rows.iter()
                .any(|row| row.len() == args.len() && row.iter().zip(args).all(|(a, b)| a == b))
        })
    }

    /// List all known predicate names.
    pub fn predicate_names(&self) -> Vec<&String> {
        self.predicates.keys().collect()
    }
}

// ══════════════════════════════════════════════════════════════════
// Reasoning engine
// ══════════════════════════════════════════════════════════════════

/// Run the Nemo Datalog engine on a program string.
///
/// Uses the `nemo::api` convenience functions to:
/// 1. Parse + load the program
/// 2. Run the chase algorithm
/// 3. Extract all derived predicate results
pub async fn reason(program_text: &str) -> Result<ReasoningResult, LedgerError> {
    if program_text
        .lines()
        .all(|l| l.trim().is_empty() || l.trim().starts_with('%'))
    {
        return Ok(ReasoningResult::default());
    }

    // Load and execute
    let mut engine = nemo::api::load_string(program_text.to_string())
        .await
        .map_err(|e| LedgerError::Reasoning(format!("Load error: {e}")))?;

    nemo::api::reason(&mut engine)
        .await
        .map_err(|e| LedgerError::Reasoning(format!("Reasoning error: {e}")))?;

    // Collect results
    let mut results: HashMap<String, Vec<Vec<String>>> = HashMap::new();

    // From exports
    let export_tags: Vec<_> = engine.exports().into_iter().map(|(t, _)| t).collect();
    for tag in &export_tags {
        query_predicate(&mut engine, tag, &mut results).await?;
    }

    // From derived predicates (rule heads)
    let derived_tags: Vec<_> = engine
        .chase_program()
        .derived_predicates()
        .iter()
        .cloned()
        .collect();
    for tag in &derived_tags {
        if !results.contains_key(&tag.to_string()) {
            query_predicate(&mut engine, tag, &mut results).await?;
        }
    }

    let total = results.values().map(|v| v.len()).sum();
    Ok(ReasoningResult {
        predicates: results,
        total_facts: total,
    })
}

/// Query a single predicate from the engine into the results map.
async fn query_predicate<
    S: nemo::execution::selection_strategy::strategy::RuleSelectionStrategy,
>(
    engine: &mut nemo::execution::ExecutionEngine<S>,
    tag: &nemo::rule_model::components::tag::Tag,
    results: &mut HashMap<String, Vec<Vec<String>>>,
) -> Result<(), LedgerError> {
    let rows = engine
        .predicate_rows(tag)
        .await
        .map_err(|e| LedgerError::Reasoning(format!("Query error for {tag}: {e}")))?;

    if let Some(iter) = rows {
        let string_rows: Vec<Vec<String>> = iter
            .map(|row| {
                row.into_iter()
                    .map(|v| strip_iri_brackets(&v.canonical_string()))
                    .collect()
            })
            .collect();
        if !string_rows.is_empty() {
            results.insert(tag.to_string(), string_rows);
        }
    }
    Ok(())
}

/// Strip `<>` IRI brackets from Nemo output for cleaner results.
fn strip_iri_brackets(s: &str) -> String {
    if s.starts_with('<') && s.ends_with('>') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ══════════════════════════════════════════════════════════════════
// Program assembly
// ══════════════════════════════════════════════════════════════════

/// Assemble a complete reasoning program from parts.
pub fn assemble_program(
    entity_facts: &str,
    graph_facts: &str,
    goal_facts: &[String],
    secret_facts: &[String],
    entity_rules: &str,
    goal_rules: &str,
    secret_rules: &str,
    export_predicates: &[&str],
) -> String {
    let mut program = String::new();

    program.push_str("% === Entity Facts ===\n");
    program.push_str(entity_facts);
    program.push('\n');

    program.push_str("% === Graph Facts ===\n");
    program.push_str(graph_facts);
    program.push('\n');

    if !goal_facts.is_empty() {
        program.push_str(&goal_facts.join("\n"));
        program.push('\n');
    }

    if !secret_facts.is_empty() {
        program.push_str(&secret_facts.join("\n"));
        program.push('\n');
    }

    program.push_str("% === Rules ===\n");
    program.push_str(entity_rules);
    program.push('\n');
    program.push_str(goal_rules);
    program.push('\n');
    program.push_str(secret_rules);
    program.push('\n');

    // Exports
    program.push_str("% === Exports ===\n");
    for pred in export_predicates {
        program.push_str(&format!("@export {pred} :- csv{{}}.\n"));
    }

    program
}

/// Default predicates to export for narrative reasoning.
pub fn default_exports() -> Vec<&'static str> {
    vec![
        "can_meet",
        "enemy",
        "danger",
        "reachable",
        "suspense",
        "active_conflict",
        "unblocked",
        "dramatic_irony",
    ]
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_result_contains() {
        let mut result = ReasoningResult::default();
        result
            .predicates
            .insert("can_meet".into(), vec![vec!["kian".into(), "nova".into()]]);
        result.total_facts = 1;

        assert!(result.contains("can_meet", &["kian", "nova"]));
        assert!(!result.contains("can_meet", &["nova", "kian"]));
        assert!(!result.contains("enemy", &["kian", "nova"]));
    }

    #[test]
    fn assemble_program_structure() {
        let program = assemble_program(
            "character(kian).\nat(kian, wasteland).",
            "connected(wasteland, oasis).",
            &["want(kian, survive, \"找水\").".to_string()],
            &[],
            "can_meet(?A, ?B) :- at(?A, ?P), at(?B, ?P), ?A != ?B.",
            "",
            "",
            &["can_meet"],
        );
        assert!(program.contains("character(kian)"));
        assert!(program.contains("can_meet"));
        assert!(program.contains("@export can_meet"));
    }

    #[tokio::test]
    async fn reason_simple_program() {
        let program = r#"
            character(kian).
            character(nova).
            at(kian, wasteland).
            at(nova, wasteland).

            can_meet(?A, ?B) :- at(?A, ?P), at(?B, ?P), ?A != ?B, character(?A), character(?B).

            @export can_meet :- csv{}.
        "#;

        let result = reason(program).await.unwrap();
        assert!(result.has("can_meet"));
        // Both (kian, nova) and (nova, kian) should be derived
        assert!(result.total_facts >= 2);
    }

    #[tokio::test]
    async fn reason_empty_program() {
        let result = reason("% empty\n").await.unwrap();
        assert_eq!(result.total_facts, 0);
    }
}
