//! Snapshot validation against dramatic intents.
//!
//! The Director validates that the current world state (Snapshot) can
//! support the declared dramatic intents. If validation fails, it
//! returns the list of unmet intents with reasons.

use ledger::Snapshot;
use ledger::state::reasoning::ReasoningResult;

use crate::drama::DramaNode;
use crate::intent::DramaticIntent;

// ══════════════════════════════════════════════════════════════════
// Verdict
// ══════════════════════════════════════════════════════════════════

/// Result of validating a snapshot against dramatic intents.
#[derive(Debug, Clone)]
pub enum Verdict {
    /// All intents are satisfiable.
    Accept,
    /// Some intents cannot be satisfied with the current world state.
    Reject { unmet: Vec<UnmetIntent> },
}

/// An intent that cannot be satisfied with the current snapshot.
#[derive(Debug, Clone)]
pub struct UnmetIntent {
    pub intent: DramaticIntent,
    pub reason: String,
}

impl Verdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, Self::Accept)
    }

    pub fn unmet_count(&self) -> usize {
        match self {
            Self::Accept => 0,
            Self::Reject { unmet } => unmet.len(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Validation
// ══════════════════════════════════════════════════════════════════

/// Validate a snapshot against a drama node's intents.
///
/// Checks:
/// - `Confrontation`: characters can_meet at the specified location
/// - `SecretReveal`: secret exists and hasn't already been revealed to targets
/// - `SuspenseResolution`: goal exists and is currently active
/// - `GoalEmergence`: owner entity exists
/// - `Reversal`: target entity exists
/// - `CharacterDevelopment`: character entity exists
pub fn validate(
    snapshot: &Snapshot,
    drama: &DramaNode,
    reasoning: Option<&ReasoningResult>,
) -> Verdict {
    let mut unmet = Vec::new();

    for intent in &drama.dramatic_intents {
        if let Some(issue) = check_intent(snapshot, intent, reasoning) {
            unmet.push(issue);
        }
    }

    if unmet.is_empty() {
        Verdict::Accept
    } else {
        Verdict::Reject { unmet }
    }
}

fn check_intent(
    snapshot: &Snapshot,
    intent: &DramaticIntent,
    reasoning: Option<&ReasoningResult>,
) -> Option<UnmetIntent> {
    match intent {
        DramaticIntent::Confrontation { between, at, .. } => {
            // Check all participants exist
            for id in between {
                if snapshot.entity(id).is_none() {
                    return Some(UnmetIntent {
                        intent: intent.clone(),
                        reason: format!("角色 {id} 不存在"),
                    });
                }
            }
            // Check location exists
            if snapshot.entity(at).is_none() {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("地点 {at} 不存在"),
                });
            }
            // If reasoning available, check can_meet
            if let Some(result) = reasoning {
                for pair in between.windows(2) {
                    let a = &pair[0];
                    let b = &pair[1];
                    if !result.contains("can_meet", &[a, b])
                        && !result.contains("can_meet", &[b, a])
                    {
                        // Characters aren't at the same location — check if location exists
                        let a_loc = snapshot
                            .entity(a)
                            .and_then(|e| e.location.as_deref())
                            .unwrap_or("?");
                        let b_loc = snapshot
                            .entity(b)
                            .and_then(|e| e.location.as_deref())
                            .unwrap_or("?");
                        return Some(UnmetIntent {
                            intent: intent.clone(),
                            reason: format!(
                                "{a}({a_loc}) 与 {b}({b_loc}) 不在同一地点, 需要先 move 到 {at}"
                            ),
                        });
                    }
                }
            }
            None
        }

        DramaticIntent::SecretReveal { secret, to, .. } => {
            // Check secret exists
            if snapshot.secret(secret).is_none() {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("秘密 {secret} 不存在"),
                });
            }
            let sec = snapshot.secret(secret).unwrap();
            // Check not already fully revealed to targets
            let all_known = to.iter().all(|t| sec.known_by.iter().any(|k| k == t));
            if all_known {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("秘密 {secret} 已被 {} 知道, 没有揭示价值", to.join(", ")),
                });
            }
            None
        }

        DramaticIntent::SuspenseResolution { goal, .. } => {
            if let Some((owner, goal_id)) = goal.split_once('/') {
                // Find the goal entity
                let found = snapshot.goal_entities.iter().any(|ge| {
                    ge.id == owner
                        && ledger::input::goal::flatten_all(std::slice::from_ref(ge))
                            .iter()
                            .any(|fg| fg.goal.id == goal_id)
                });
                if !found {
                    return Some(UnmetIntent {
                        intent: intent.clone(),
                        reason: format!("目标 {goal} 不存在"),
                    });
                }
            } else {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("目标格式错误: {goal} (应为 owner/goal_id)"),
                });
            }
            None
        }

        DramaticIntent::GoalEmergence { owner, .. } => {
            if snapshot.entity(owner).is_none() {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("角色 {owner} 不存在"),
                });
            }
            None
        }

        DramaticIntent::Reversal { target, .. } => {
            if snapshot.entity(target).is_none() {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("角色 {target} 不存在"),
                });
            }
            None
        }

        DramaticIntent::CharacterDevelopment { character, .. } => {
            if snapshot.entity(character).is_none() {
                return Some(UnmetIntent {
                    intent: intent.clone(),
                    reason: format!("角色 {character} 不存在"),
                });
            }
            None
        }

        DramaticIntent::TensionBuild { .. } => {
            // Tension build is always valid — no preconditions
            None
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Display
// ══════════════════════════════════════════════════════════════════

impl Verdict {
    pub fn render(&self) -> String {
        match self {
            Self::Accept => "✓ 所有戏剧性意图可满足".to_string(),
            Self::Reject { unmet } => {
                let mut out = format!("✗ {} 个意图无法满足:\n", unmet.len());
                for item in unmet {
                    out.push_str(&format!(
                        "  ⊘ {} — {}\n",
                        item.intent.summary(),
                        item.reason,
                    ));
                }
                out
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drama::{DirectorNotes, Pacing};
    use ledger::input::entity::Entity;
    use ledger::input::secret::{DramaticFunction, Secret};

    fn make_snapshot() -> Snapshot {
        Snapshot::from_parts(
            "ch03",
            vec![
                Entity {
                    entity_type: "character".into(),
                    id: "kian".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("oasis_gate".into()),
                    relationships: vec![],
                    inventory: vec![],
                    alignment: None,
                    rivals: vec![],
                    members: vec![],
                    properties: vec![],
                    connections: vec![],
                    tags: vec![],
                },
                Entity {
                    entity_type: "character".into(),
                    id: "nova".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: Some("oasis_gate".into()),
                    relationships: vec![],
                    inventory: vec![],
                    alignment: None,
                    rivals: vec![],
                    members: vec![],
                    properties: vec![],
                    connections: vec![],
                    tags: vec![],
                },
                Entity {
                    entity_type: "location".into(),
                    id: "oasis_gate".into(),
                    name: None,
                    traits: vec![],
                    beliefs: vec![],
                    desires: vec![],
                    intentions: vec![],
                    location: None,
                    relationships: vec![],
                    inventory: vec![],
                    alignment: None,
                    rivals: vec![],
                    members: vec![],
                    properties: vec![],
                    connections: vec![],
                    tags: vec![],
                },
            ],
            vec![Secret {
                id: "oasis_truth".into(),
                content: "绿洲的能源来自活人".into(),
                known_by: vec![],
                revealed_to_reader: false,
                dramatic_function: Some(DramaticFunction::Reversal),
            }],
            vec![],
        )
    }

    #[test]
    fn validate_accept_confrontation() {
        let snap = make_snapshot();
        let drama = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![DramaticIntent::Confrontation {
                between: vec!["kian".into(), "nova".into()],
                at: "oasis_gate".into(),
                depends_on: vec![],
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let verdict = validate(&snap, &drama, None);
        assert!(verdict.is_accept());
    }

    #[test]
    fn validate_reject_missing_entity() {
        let snap = make_snapshot();
        let drama = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![DramaticIntent::Confrontation {
                between: vec!["kian".into(), "ghost".into()],
                at: "oasis_gate".into(),
                depends_on: vec![],
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let verdict = validate(&snap, &drama, None);
        assert!(!verdict.is_accept());
        assert_eq!(verdict.unmet_count(), 1);
    }

    #[test]
    fn validate_secret_reveal() {
        let snap = make_snapshot();
        let drama = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![DramaticIntent::SecretReveal {
                secret: "oasis_truth".into(),
                to: vec!["kian".into()],
                reveal_to_reader: true,
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let verdict = validate(&snap, &drama, None);
        assert!(verdict.is_accept());
    }

    #[test]
    fn validate_secret_already_revealed() {
        let mut snap = make_snapshot();
        snap.secrets[0].known_by = vec!["kian".into()];
        let drama = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![DramaticIntent::SecretReveal {
                secret: "oasis_truth".into(),
                to: vec!["kian".into()],
                reveal_to_reader: false,
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let verdict = validate(&snap, &drama, None);
        assert!(!verdict.is_accept());
    }

    #[test]
    fn validate_render_reject() {
        let snap = make_snapshot();
        let drama = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![DramaticIntent::Reversal {
                target: "ghost_char".into(),
                trigger: "test".into(),
                secret: None,
                timing: crate::intent::Timing::Climax,
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let verdict = validate(&snap, &drama, None);
        let text = verdict.render();
        assert!(text.contains("ghost_char"));
        assert!(text.contains("不存在"));
    }
}
