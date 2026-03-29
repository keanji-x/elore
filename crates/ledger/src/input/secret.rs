//! Information disclosure layer — secrets and narrative techniques.
//!
//! Controls "who knows what, when" for suspense, dramatic irony,
//! misdirection, and reveals. Secrets are loaded from YAML and
//! translated into Datalog facts for reasoning.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LedgerError;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// The dramatic purpose of a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DramaticFunction {
    Reversal,
    Suspense,
    DramaticIrony,
    Misdirection,
    Foreshadowing,
}

/// A narrative secret with disclosure tracking.
///
/// Secrets enable formal reasoning about narrative techniques:
///
/// | Technique        | known_by    | revealed_to_reader |
/// |------------------|-------------|--------------------|
/// | **悬疑**          | nobody      | false              |
/// | **戏剧性反讽**    | some chars  | true               |
/// | **扮猪吃老虎**    | self only   | false              |
/// | **误导**          | wrong belief| true               |
/// | **反转/揭示**     | → revealed  | true               |
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Secret {
    pub id: String,
    pub content: String,

    /// Which characters currently know this secret.
    #[serde(default)]
    pub known_by: Vec<String>,

    /// Whether the reader (audience) knows this secret.
    #[serde(default)]
    pub revealed_to_reader: bool,

    /// The intended dramatic purpose.
    #[serde(default)]
    pub dramatic_function: Option<DramaticFunction>,
}

/// The active narrative technique implied by a secret's disclosure state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarrativeTechnique {
    /// Nobody knows, reader doesn't know → mystery/suspense
    Suspense,
    /// Some characters know, reader knows, others don't → dramatic irony
    DramaticIrony,
    /// Character knows, reader doesn't → hidden advantage
    HiddenAdvantage,
    /// Wrong belief, reader is also misled → misdirection
    Misdirection,
    /// Everyone relevant knows → resolved / no longer secret
    Resolved,
}

// ══════════════════════════════════════════════════════════════════
// Loading
// ══════════════════════════════════════════════════════════════════

/// Container for secrets.yaml which holds a list of secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SecretsFile {
    #[serde(default)]
    pub(super) secrets: Vec<Secret>,
}

/// Load secrets from `.everlore/entities/secrets.yaml`.
pub fn load_secrets(dir: &Path) -> Result<Vec<Secret>, LedgerError> {
    let path = dir.join("secrets.yaml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let file: SecretsFile = serde_yaml::from_str(&content)?;
    Ok(file.secrets)
}

// ══════════════════════════════════════════════════════════════════
// Analysis
// ══════════════════════════════════════════════════════════════════

impl Secret {
    /// Classify the current narrative technique based on disclosure state.
    pub fn classify(&self) -> NarrativeTechnique {
        let nobody_knows = self.known_by.is_empty();
        let reader_knows = self.revealed_to_reader;

        match (nobody_knows, reader_knows) {
            (true, false) => NarrativeTechnique::Suspense,
            (true, true) => NarrativeTechnique::DramaticIrony, // reader knows something no char does
            (false, false) => NarrativeTechnique::HiddenAdvantage,
            (false, true) => NarrativeTechnique::DramaticIrony,
        }
    }

    /// Apply a `reveal` effect: add a character to `known_by`.
    pub fn reveal_to(&mut self, character: &str) {
        if !self.known_by.iter().any(|c| c == character) {
            self.known_by.push(character.to_string());
        }
    }

    /// Apply a `reveal_to_reader` effect.
    pub fn reveal_to_reader(&mut self) {
        self.revealed_to_reader = true;
    }
}

// ══════════════════════════════════════════════════════════════════
// Datalog translation
// ══════════════════════════════════════════════════════════════════

/// Translate secrets into Datalog facts.
pub fn secrets_to_datalog(secrets: &[Secret]) -> Vec<String> {
    let mut facts = vec!["% === Secret Facts ===".to_string()];

    for secret in secrets {
        let sid = &secret.id;
        facts.push(format!("secret({sid})."));

        for char_id in &secret.known_by {
            facts.push(format!("secret_known_by({sid}, {char_id})."));
        }

        if secret.revealed_to_reader {
            facts.push(format!("secret_revealed_to_reader({sid})."));
        }

        // Generate `secret_not_known_by` for all characters not in known_by.
        // This requires knowing the full character list, so we emit a rule instead:
        // The actual negation is handled by the Datalog rule `dramatic_irony`.
    }

    facts
}

/// Built-in rules for information asymmetry reasoning.
pub fn secret_rules() -> &'static str {
    r#"% === Information Asymmetry Rules ===

% Dramatic irony: reader and some char know, but another char doesn't
dramatic_irony(?Secret, ?Uninformed) :-
  secret_known_by(?Secret, ?Informed),
  secret_revealed_to_reader(?Secret),
  character(?Uninformed),
  ?Informed != ?Uninformed,
  ~secret_known_by(?Secret, ?Uninformed).
"#
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_secret() -> Secret {
        Secret {
            id: "oasis_truth".into(),
            content: "绿洲的能源来自活人培养皿".into(),
            known_by: vec![],
            revealed_to_reader: false,
            dramatic_function: Some(DramaticFunction::Reversal),
        }
    }

    #[test]
    fn classify_suspense() {
        let s = make_secret();
        assert_eq!(s.classify(), NarrativeTechnique::Suspense);
    }

    #[test]
    fn classify_dramatic_irony_after_reveal() {
        let mut s = make_secret();
        s.reveal_to("kian");
        s.reveal_to_reader();
        assert_eq!(s.classify(), NarrativeTechnique::DramaticIrony);
    }

    #[test]
    fn classify_hidden_advantage() {
        let mut s = make_secret();
        s.reveal_to("kian");
        assert_eq!(s.classify(), NarrativeTechnique::HiddenAdvantage);
    }

    #[test]
    fn reveal_is_idempotent() {
        let mut s = make_secret();
        s.reveal_to("kian");
        s.reveal_to("kian");
        assert_eq!(s.known_by.len(), 1);
    }

    #[test]
    fn secrets_to_datalog_generates_facts() {
        let secrets = vec![make_secret()];
        let facts = secrets_to_datalog(&secrets);
        assert!(facts.iter().any(|f| f == "secret(oasis_truth)."));
    }

    /// Regression: Bug 1 — DramaticFunction was serialized as "dramaticirony"
    /// (lowercase) instead of "dramatic_irony" (snake_case). Fixed by changing
    /// `#[serde(rename_all = "lowercase")]` → `#[serde(rename_all = "snake_case")]`.
    #[test]
    fn dramatic_function_serde_snake_case() {
        // Deserialize snake_case (natural user input)
        let json = r#"{"id":"s","content":"x","dramatic_function":"dramatic_irony"}"#;
        let s: Secret = serde_json::from_str(json).unwrap();
        assert_eq!(s.dramatic_function, Some(DramaticFunction::DramaticIrony));

        // Re-serialize must produce snake_case
        let out = serde_json::to_string(&s.dramatic_function).unwrap();
        assert_eq!(out, "\"dramatic_irony\"");

        // Other variants should be lowercase with underscore where applicable
        let json2 = r#""foreshadowing""#;
        let df: DramaticFunction = serde_json::from_str(json2).unwrap();
        assert_eq!(df, DramaticFunction::Foreshadowing);
    }

    /// Regression: YAML deserialization of secrets.yaml must handle snake_case.
    #[test]
    fn secrets_yaml_dramatic_function_snake_case() {
        let yaml = r#"
secrets:
  - id: vex_secret
    content: "维克斯为辛迪加工作"
    known_by: [vex]
    dramatic_function: dramatic_irony
  - id: nav_secret
    content: "导航数据"
    dramatic_function: foreshadowing
"#;
        let file: super::SecretsFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(file.secrets.len(), 2);
        assert_eq!(
            file.secrets[0].dramatic_function,
            Some(DramaticFunction::DramaticIrony)
        );
        assert_eq!(
            file.secrets[1].dramatic_function,
            Some(DramaticFunction::Foreshadowing)
        );
    }
}
