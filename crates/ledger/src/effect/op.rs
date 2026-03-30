//! Effect operations — the atomic state mutation vocabulary.
//!
//! Each `Op` represents a single, reversible state change.
//! Ops are serialized to JSONL in `history.jsonl` and can be
//! parsed from a DSL string like `remove_item(kian, 电磁短刀)`.

use serde::{Deserialize, Serialize};

use crate::LedgerError;
use crate::input::entity::{Entity, Relationship};
use crate::input::goal::{GoalEntity, GoalStatus};
use crate::input::secret::Secret;

// ══════════════════════════════════════════════════════════════════
// Op enum
// ══════════════════════════════════════════════════════════════════

/// A single atomic effect operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op")]
pub enum Op {
    // ── Entity CRUD ──────────────────────────────────────────────
    #[serde(rename = "add_trait")]
    AddTrait { entity: String, value: String },
    #[serde(rename = "remove_trait")]
    RemoveTrait { entity: String, value: String },
    #[serde(rename = "add_item")]
    AddItem { entity: String, item: String },
    #[serde(rename = "remove_item")]
    RemoveItem { entity: String, item: String },
    #[serde(rename = "move")]
    Move { entity: String, location: String },
    #[serde(rename = "add_rel")]
    AddRel {
        entity: String,
        target: String,
        rel: String,
    },
    #[serde(rename = "remove_rel")]
    RemoveRel { entity: String, target: String },
    #[serde(rename = "set_belief")]
    SetBelief {
        entity: String,
        old: String,
        new: String,
    },
    #[serde(rename = "add_desire")]
    AddDesire { entity: String, value: String },
    #[serde(rename = "remove_desire")]
    RemoveDesire { entity: String, value: String },

    // ── Information disclosure ────────────────────────────────────
    #[serde(rename = "reveal")]
    Reveal { secret: String, to: String },
    #[serde(rename = "reveal_to_reader")]
    RevealToReader { secret: String },

    // ── Desire engine ────────────────────────────────────────────
    #[serde(rename = "resolve_goal")]
    ResolveGoal {
        owner: String,
        goal_id: String,
        solution: String,
    },
    #[serde(rename = "fail_goal")]
    FailGoal { owner: String, goal_id: String },
    #[serde(rename = "emerge_goal")]
    EmergeGoal {
        owner: String,
        goal_id: String,
        want: String,
        #[serde(default)]
        problem: Option<String>,
    },
}

impl Op {
    /// Get the primary entity ID this effect targets (if applicable).
    pub fn entity_id(&self) -> Option<&str> {
        match self {
            Self::AddTrait { entity, .. }
            | Self::RemoveTrait { entity, .. }
            | Self::AddItem { entity, .. }
            | Self::RemoveItem { entity, .. }
            | Self::Move { entity, .. }
            | Self::AddRel { entity, .. }
            | Self::RemoveRel { entity, .. }
            | Self::SetBelief { entity, .. }
            | Self::AddDesire { entity, .. }
            | Self::RemoveDesire { entity, .. } => Some(entity),

            Self::Reveal { .. } | Self::RevealToReader { .. } => None,

            Self::ResolveGoal { owner, .. }
            | Self::FailGoal { owner, .. }
            | Self::EmergeGoal { owner, .. } => Some(owner),
        }
    }

    /// Apply this effect to an entity. Returns true if applied.
    pub fn apply_to_entity(&self, entity: &mut Entity) -> bool {
        let Some(target_id) = self.entity_id() else {
            return false;
        };
        if entity.id() != target_id {
            return false;
        }

        let Some(c) = entity.as_character_mut() else {
            // Only character entities support these mutations
            return false;
        };

        match self {
            Self::AddTrait { value, .. } => {
                if !c.traits.contains(value) {
                    c.traits.push(value.clone());
                }
            }
            Self::RemoveTrait { value, .. } => {
                c.traits.retain(|t| t != value);
            }
            Self::AddItem { item, .. } => {
                if !c.inventory.contains(item) {
                    c.inventory.push(item.clone());
                }
            }
            Self::RemoveItem { item, .. } => {
                c.inventory.retain(|i| i != item);
            }
            Self::Move { location, .. } => {
                c.location = Some(location.clone());
            }
            Self::AddRel { target, rel, .. } => {
                if !c
                    .relationships
                    .iter()
                    .any(|r| r.target == *target && r.rel == *rel)
                {
                    c.relationships.push(Relationship {
                        target: target.clone(),
                        rel: rel.clone(),
                    });
                }
            }
            Self::RemoveRel { target, .. } => {
                c.relationships.retain(|r| r.target != *target);
            }
            Self::SetBelief { old, new, .. } => {
                if let Some(b) = c.beliefs.iter_mut().find(|b| **b == *old) {
                    *b = new.clone();
                } else {
                    c.beliefs.push(new.clone());
                }
            }
            Self::AddDesire { value, .. } => {
                if !c.desires.contains(value) {
                    c.desires.push(value.clone());
                }
            }
            Self::RemoveDesire { value, .. } => {
                c.desires.retain(|d| d != value);
            }
            // Non-entity ops
            _ => return false,
        }
        true
    }

    /// Apply this effect to a secret. Returns true if applied.
    pub fn apply_to_secret(&self, secret: &mut Secret) -> bool {
        match self {
            Self::Reveal { secret: sid, to } if *sid == secret.id => {
                secret.reveal_to(to);
                true
            }
            Self::RevealToReader { secret: sid } if *sid == secret.id => {
                secret.reveal_to_reader();
                true
            }
            _ => false,
        }
    }

    /// Apply this effect to a goal entity. Returns true if applied.
    pub fn apply_to_goal(&self, goal_entity: &mut GoalEntity) -> bool {
        match self {
            Self::ResolveGoal {
                owner,
                goal_id,
                solution,
            } if *owner == goal_entity.id => {
                if let Some(goal) = find_goal_mut(&mut goal_entity.goals, goal_id) {
                    goal.status = GoalStatus::Resolved;
                    goal.solution = Some(solution.clone());
                    return true;
                }
                false
            }
            Self::FailGoal { owner, goal_id } if *owner == goal_entity.id => {
                if let Some(goal) = find_goal_mut(&mut goal_entity.goals, goal_id) {
                    goal.status = GoalStatus::Failed;
                    return true;
                }
                false
            }
            Self::EmergeGoal {
                owner,
                goal_id,
                want,
                problem,
            } if *owner == goal_entity.id => {
                use crate::input::goal::Goal;
                goal_entity.goals.push(Goal {
                    id: goal_id.clone(),
                    want: want.clone(),
                    problem: problem.clone(),
                    solution: None,
                    status: GoalStatus::Active,
                    blocked_by: vec![],
                    conflicts_with: vec![],
                    side_effects: vec![],
                    children: vec![],
                });
                true
            }
            _ => false,
        }
    }

    /// Human-readable description (Chinese).
    pub fn describe(&self) -> String {
        match self {
            Self::AddTrait { entity, value } => format!("{entity}: 获得特质 \"{value}\""),
            Self::RemoveTrait { entity, value } => format!("{entity}: 失去特质 \"{value}\""),
            Self::AddItem { entity, item } => format!("{entity}: 获得物品 \"{item}\""),
            Self::RemoveItem { entity, item } => format!("{entity}: 失去物品 \"{item}\""),
            Self::Move { entity, location } => format!("{entity}: 移动至 {location}"),
            Self::AddRel {
                entity,
                target,
                rel,
            } => format!("{entity}: 新关系 {rel}({target})"),
            Self::RemoveRel { entity, target } => {
                format!("{entity}: 断绝与 {target} 的关系")
            }
            Self::SetBelief {
                entity, old, new, ..
            } => format!("{entity}: 信念 \"{old}\" → \"{new}\""),
            Self::AddDesire { entity, value } => format!("{entity}: 新欲望 \"{value}\""),
            Self::RemoveDesire { entity, value } => format!("{entity}: 放弃欲望 \"{value}\""),
            Self::Reveal { secret, to } => format!("秘密 {secret} 揭示给 {to}"),
            Self::RevealToReader { secret } => format!("秘密 {secret} 揭示给读者"),
            Self::ResolveGoal {
                owner,
                goal_id,
                solution,
            } => format!("{owner}/{goal_id}: 解决 — \"{solution}\""),
            Self::FailGoal { owner, goal_id } => format!("{owner}/{goal_id}: 失败"),
            Self::EmergeGoal {
                owner,
                goal_id,
                want,
                ..
            } => format!("{owner}: 新目标 {goal_id} — \"{want}\""),
        }
    }

    /// Extract all entity IDs targeted by this effect to validate if they exist.
    pub fn extract_entities(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        match self {
            Self::AddTrait { entity, .. }
            | Self::RemoveTrait { entity, .. }
            | Self::AddItem { entity, .. }
            | Self::RemoveItem { entity, .. }
            | Self::Move { entity, .. }
            | Self::SetBelief { entity, .. }
            | Self::AddDesire { entity, .. }
            | Self::RemoveDesire { entity, .. } => {
                ids.push(entity.as_str());
            }
            Self::AddRel { entity, target, .. } | Self::RemoveRel { entity, target } => {
                ids.push(entity.as_str());
                ids.push(target.as_str());
            }
            Self::Reveal { to, .. } => {
                ids.push(to.as_str());
            }
            Self::RevealToReader { .. } => {}
            Self::ResolveGoal { owner, .. }
            | Self::FailGoal { owner, .. }
            | Self::EmergeGoal { owner, .. } => {
                ids.push(owner.as_str());
            }
        }
        ids
    }

    /// Extract all secret IDs targeted by this effect to validate if they exist.
    pub fn extract_secrets(&self) -> Vec<&str> {
        match self {
            Self::Reveal { secret, .. } | Self::RevealToReader { secret } => vec![secret.as_str()],
            _ => vec![],
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// DSL parser
// ══════════════════════════════════════════════════════════════════

impl Op {
    /// Parse a DSL string like `remove_item(kian, 电磁短刀)`.
    pub fn parse(s: &str) -> Result<Self, LedgerError> {
        let s = s.trim();
        let paren_start = s
            .find('(')
            .ok_or_else(|| LedgerError::EffectParse(format!("Missing '(' in: {s}")))?;
        let paren_end = s
            .rfind(')')
            .ok_or_else(|| LedgerError::EffectParse(format!("Missing ')' in: {s}")))?;

        let op = &s[..paren_start];
        let inner = &s[paren_start + 1..paren_end];
        let args: Vec<&str> = inner.splitn(3, ',').map(|a| a.trim()).collect();

        match op {
            "add_trait" => {
                check_args(op, &args, 2)?;
                Ok(Self::AddTrait {
                    entity: args[0].into(),
                    value: args[1].into(),
                })
            }
            "remove_trait" => {
                check_args(op, &args, 2)?;
                Ok(Self::RemoveTrait {
                    entity: args[0].into(),
                    value: args[1].into(),
                })
            }
            "add_item" => {
                check_args(op, &args, 2)?;
                Ok(Self::AddItem {
                    entity: args[0].into(),
                    item: args[1].into(),
                })
            }
            "remove_item" => {
                check_args(op, &args, 2)?;
                Ok(Self::RemoveItem {
                    entity: args[0].into(),
                    item: args[1].into(),
                })
            }
            "move" => {
                check_args(op, &args, 2)?;
                Ok(Self::Move {
                    entity: args[0].into(),
                    location: args[1].into(),
                })
            }
            "add_rel" => {
                check_args(op, &args, 3)?;
                Ok(Self::AddRel {
                    entity: args[0].into(),
                    target: args[1].into(),
                    rel: args[2].into(),
                })
            }
            "remove_rel" => {
                check_args(op, &args, 2)?;
                Ok(Self::RemoveRel {
                    entity: args[0].into(),
                    target: args[1].into(),
                })
            }
            "set_belief" => {
                check_args(op, &args, 3)?;
                Ok(Self::SetBelief {
                    entity: args[0].into(),
                    old: args[1].into(),
                    new: args[2].into(),
                })
            }
            "add_desire" => {
                check_args(op, &args, 2)?;
                Ok(Self::AddDesire {
                    entity: args[0].into(),
                    value: args[1].into(),
                })
            }
            "remove_desire" => {
                check_args(op, &args, 2)?;
                Ok(Self::RemoveDesire {
                    entity: args[0].into(),
                    value: args[1].into(),
                })
            }
            "reveal" => {
                check_args(op, &args, 2)?;
                Ok(Self::Reveal {
                    secret: args[0].into(),
                    to: args[1].into(),
                })
            }
            "reveal_to_reader" => {
                check_args(op, &args, 1)?;
                Ok(Self::RevealToReader {
                    secret: args[0].into(),
                })
            }
            "resolve_goal" => {
                check_args(op, &args, 3)?;
                Ok(Self::ResolveGoal {
                    owner: args[0].into(),
                    goal_id: args[1].into(),
                    solution: args[2].into(),
                })
            }
            "fail_goal" => {
                check_args(op, &args, 2)?;
                Ok(Self::FailGoal {
                    owner: args[0].into(),
                    goal_id: args[1].into(),
                })
            }
            "emerge_goal" => {
                check_args(op, &args, 3)?;
                Ok(Self::EmergeGoal {
                    owner: args[0].into(),
                    goal_id: args[1].into(),
                    want: args[2].into(),
                    problem: None,
                })
            }
            _ => Err(LedgerError::EffectParse(format!("Unknown op: {op}"))),
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

fn check_args(op: &str, args: &[&str], required: usize) -> Result<(), LedgerError> {
    if args.len() < required {
        Err(LedgerError::EffectParse(format!(
            "{op} requires {required} args, got {}",
            args.len()
        )))
    } else {
        Ok(())
    }
}

/// Recursively find a goal by ID in a goal tree.
fn find_goal_mut<'a>(
    goals: &'a mut [crate::input::goal::Goal],
    goal_id: &str,
) -> Option<&'a mut crate::input::goal::Goal> {
    for goal in goals.iter_mut() {
        if goal.id == goal_id {
            return Some(goal);
        }
        if let Some(found) = find_goal_mut(&mut goal.children, goal_id) {
            return Some(found);
        }
    }
    None
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remove_item() {
        let op = Op::parse("remove_item(kian, 电磁短刀)").unwrap();
        assert_eq!(
            op,
            Op::RemoveItem {
                entity: "kian".into(),
                item: "电磁短刀".into()
            }
        );
    }

    #[test]
    fn parse_add_rel() {
        let op = Op::parse("add_rel(kian, nova, trusts)").unwrap();
        assert_eq!(
            op,
            Op::AddRel {
                entity: "kian".into(),
                target: "nova".into(),
                rel: "trusts".into()
            }
        );
    }

    #[test]
    fn parse_reveal() {
        let op = Op::parse("reveal(oasis_truth, kian)").unwrap();
        assert_eq!(
            op,
            Op::Reveal {
                secret: "oasis_truth".into(),
                to: "kian".into()
            }
        );
    }

    #[test]
    fn parse_resolve_goal() {
        let op = Op::parse("resolve_goal(kian, survive_drought, 找到了地下水)").unwrap();
        assert_eq!(
            op,
            Op::ResolveGoal {
                owner: "kian".into(),
                goal_id: "survive_drought".into(),
                solution: "找到了地下水".into()
            }
        );
    }

    #[test]
    fn apply_to_entity() {
        use crate::input::entity::Character;
        let mut entity = Entity::Character(Character {
            id: "kian".into(),
            name: None,
            traits: vec![],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: None,
            relationships: vec![],
            inventory: vec!["刀".into()],
            goals: vec![],
            tags: vec![],
            description: None,
        });

        let op = Op::RemoveItem {
            entity: "kian".into(),
            item: "刀".into(),
        };
        assert!(op.apply_to_entity(&mut entity));
        assert!(entity.as_character().unwrap().inventory.is_empty());
    }

    #[test]
    fn apply_to_wrong_entity() {
        use crate::input::entity::Character;
        let mut entity = Entity::Character(Character {
            id: "nova".into(),
            name: None,
            traits: vec![],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: None,
            relationships: vec![],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: None,
        });

        let op = Op::AddTrait {
            entity: "kian".into(),
            value: "test".into(),
        };
        assert!(!op.apply_to_entity(&mut entity));
    }

    #[test]
    fn describe_chinese() {
        let op = Op::RemoveItem {
            entity: "kian".into(),
            item: "旧式防毒面具".into(),
        };
        assert_eq!(op.describe(), "kian: 失去物品 \"旧式防毒面具\"");
    }

    #[test]
    fn serialize_roundtrip() {
        let op = Op::Reveal {
            secret: "oasis_truth".into(),
            to: "kian".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
    }
}
