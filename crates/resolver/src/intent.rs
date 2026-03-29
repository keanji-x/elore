//! Dramatic intent definitions.
//!
//! A `DramaticIntent` is what the Director **wants** to happen in a chapter.
//! Intents are declarative — they describe goals, not implementation.
//! The Director validates whether the world snapshot allows each intent,
//! and encodes them into the Author prompt.

use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════
// Core intent types
// ══════════════════════════════════════════════════════════════════

/// A single dramatic intent — what the Director wants this chapter to achieve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DramaticIntent {
    /// Two or more characters meet and confront each other.
    #[serde(rename = "confrontation")]
    Confrontation {
        between: Vec<String>,
        at: String,
        #[serde(default)]
        depends_on: Vec<String>,
    },

    /// A dramatic reversal — expectation subverted.
    #[serde(rename = "reversal")]
    Reversal {
        target: String,
        trigger: String,
        #[serde(default)]
        secret: Option<String>,
        #[serde(default)]
        timing: Timing,
    },

    /// A goal reaches its conclusion (resolved or failed).
    #[serde(rename = "suspense_resolution")]
    SuspenseResolution {
        /// Format: "owner/goal_id"
        goal: String,
        expected_outcome: GoalOutcome,
    },

    /// A secret is revealed to one or more characters (and/or the reader).
    #[serde(rename = "secret_reveal")]
    SecretReveal {
        secret: String,
        to: Vec<String>,
        #[serde(default)]
        reveal_to_reader: bool,
    },

    /// A new goal emerges for a character.
    #[serde(rename = "goal_emergence")]
    GoalEmergence {
        owner: String,
        goal_id: String,
        want: String,
        #[serde(default)]
        problem: Option<String>,
    },

    /// A character undergoes internal change (belief shift, trait change).
    #[serde(rename = "character_development")]
    CharacterDevelopment {
        character: String,
        arc: String,
        #[serde(default)]
        effects: Vec<String>,
    },

    /// Tension escalation without resolution.
    #[serde(rename = "tension_build")]
    TensionBuild {
        source: String,
        #[serde(default)]
        foreshadowing: Vec<String>,
    },
}

/// Timing within the chapter for a reversal or reveal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Timing {
    Opening,
    #[default]
    BuildUp,
    Climax,
    Resolution,
}

/// Expected outcome for a goal resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GoalOutcome {
    Resolved,
    Failed,
    Blocked,
}

// ══════════════════════════════════════════════════════════════════
// Display
// ══════════════════════════════════════════════════════════════════

impl DramaticIntent {
    /// Short human-readable summary.
    pub fn summary(&self) -> String {
        match self {
            Self::Confrontation { between, at, .. } => {
                format!("对峙: {} @ {at}", between.join(" vs "))
            }
            Self::Reversal { target, trigger, .. } => {
                format!("反转: {target} — 触发: {trigger}")
            }
            Self::SuspenseResolution {
                goal,
                expected_outcome,
            } => {
                format!("悬念解决: {goal} → {expected_outcome:?}")
            }
            Self::SecretReveal { secret, to, .. } => {
                format!("揭秘: {secret} → {}", to.join(", "))
            }
            Self::GoalEmergence {
                owner,
                goal_id,
                want,
                ..
            } => {
                format!("新目标: {owner}/{goal_id} — {want}")
            }
            Self::CharacterDevelopment { character, arc, .. } => {
                format!("角色发展: {character} — {arc}")
            }
            Self::TensionBuild { source, .. } => {
                format!("张力蓄积: {source}")
            }
        }
    }

    /// Get all entity IDs referenced by this intent.
    pub fn referenced_entities(&self) -> Vec<&str> {
        match self {
            Self::Confrontation { between, at, .. } => {
                let mut ids: Vec<&str> = between.iter().map(|s| s.as_str()).collect();
                ids.push(at);
                ids
            }
            Self::Reversal { target, .. } => vec![target],
            Self::SuspenseResolution { goal, .. } => {
                if let Some((owner, _)) = goal.split_once('/') {
                    vec![owner]
                } else {
                    vec![]
                }
            }
            Self::SecretReveal { to, .. } => to.iter().map(|s| s.as_str()).collect(),
            Self::GoalEmergence { owner, .. } => vec![owner],
            Self::CharacterDevelopment { character, .. } => vec![character],
            Self::TensionBuild { .. } => vec![],
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_confrontation() {
        let intent = DramaticIntent::Confrontation {
            between: vec!["kian".into(), "nova".into()],
            at: "oasis_gate".into(),
            depends_on: vec![],
        };
        let yaml = serde_yaml::to_string(&intent).unwrap();
        assert!(yaml.contains("confrontation"));
        assert!(yaml.contains("kian"));
    }

    #[test]
    fn deserialize_secret_reveal() {
        let yaml = r#"
type: secret_reveal
secret: oasis_truth
to: [kian]
reveal_to_reader: true
"#;
        let intent: DramaticIntent = serde_yaml::from_str(yaml).unwrap();
        match intent {
            DramaticIntent::SecretReveal {
                secret,
                to,
                reveal_to_reader,
            } => {
                assert_eq!(secret, "oasis_truth");
                assert_eq!(to, vec!["kian"]);
                assert!(reveal_to_reader);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn summary_outputs() {
        let intent = DramaticIntent::Confrontation {
            between: vec!["kian".into(), "nova".into()],
            at: "oasis_gate".into(),
            depends_on: vec![],
        };
        assert!(intent.summary().contains("kian vs nova"));
    }

    #[test]
    fn referenced_entities_from_confrontation() {
        let intent = DramaticIntent::Confrontation {
            between: vec!["kian".into(), "nova".into()],
            at: "oasis_gate".into(),
            depends_on: vec![],
        };
        let refs = intent.referenced_entities();
        assert!(refs.contains(&"kian"));
        assert!(refs.contains(&"nova"));
        assert!(refs.contains(&"oasis_gate"));
    }
}
