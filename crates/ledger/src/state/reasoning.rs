//! Datalog reasoning via Nemo — the logical inference engine.
//!
//! This module contains only the Nemo execution layer and result types.
//! Program assembly lives in `program.rs`, fact generation in `fact.rs`,
//! and rule management in `rule.rs`.

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

    // ── Typed accessors ─────────────────────────────────────────

    /// Get binary predicate results as `(a, b)` pairs.
    pub fn pairs(&self, predicate: &str) -> Vec<(&str, &str)> {
        self.predicates
            .get(predicate)
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.len() >= 2)
                    .map(|r| (r[0].as_str(), r[1].as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get ternary predicate results as `(a, b, c)` triples.
    pub fn triples(&self, predicate: &str) -> Vec<(&str, &str, &str)> {
        self.predicates
            .get(predicate)
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.len() >= 3)
                    .map(|r| (r[0].as_str(), r[1].as_str(), r[2].as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get quaternary predicate results as `(a, b, c, d)` quads.
    pub fn quads(&self, predicate: &str) -> Vec<(&str, &str, &str, &str)> {
        self.predicates
            .get(predicate)
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.len() >= 4)
                    .map(|r| (r[0].as_str(), r[1].as_str(), r[2].as_str(), r[3].as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get unary predicate results as single values.
    pub fn singles(&self, predicate: &str) -> Vec<&str> {
        self.predicates
            .get(predicate)
            .map(|rows| {
                rows.iter()
                    .filter(|r| !r.is_empty())
                    .map(|r| r[0].as_str())
                    .collect()
            })
            .unwrap_or_default()
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
    fn pairs_accessor() {
        let mut result = ReasoningResult::default();
        result.predicates.insert(
            "can_meet".into(),
            vec![
                vec!["kian".into(), "nova".into()],
                vec!["nova".into(), "kian".into()],
            ],
        );
        let pairs = result.pairs("can_meet");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("kian", "nova"));
    }

    #[test]
    fn triples_accessor() {
        let mut result = ReasoningResult::default();
        result.predicates.insert(
            "betrayal_opportunity".into(),
            vec![vec!["a".into(), "b".into(), "secret1".into()]],
        );
        let triples = result.triples("betrayal_opportunity");
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0], ("a", "b", "secret1"));
    }

    #[test]
    fn singles_accessor() {
        let mut result = ReasoningResult::default();
        result.predicates.insert(
            "orphaned_secret".into(),
            vec![vec!["secret1".into()]],
        );
        let singles = result.singles("orphaned_secret");
        assert_eq!(singles, vec!["secret1"]);
    }

    #[test]
    fn missing_predicate_returns_empty() {
        let result = ReasoningResult::default();
        assert!(result.pairs("nonexistent").is_empty());
        assert!(result.triples("nonexistent").is_empty());
        assert!(result.singles("nonexistent").is_empty());
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
        assert!(result.total_facts >= 2);
    }

    #[tokio::test]
    async fn reason_empty_program() {
        let result = reason("% empty\n").await.unwrap();
        assert_eq!(result.total_facts, 0);
    }
}
