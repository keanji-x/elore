//! Audit — post-generation consistency checks.
//!
//! Verifies that the generated text + extracted effects
//! are consistent with the world snapshot.

use ledger::effect::op::Op;
use ledger::Snapshot;

/// Result of auditing a chapter's output.
#[derive(Debug, Clone)]
pub struct AuditReport {
    pub chapter: String,
    pub issues: Vec<AuditIssue>,
}

/// A single consistency issue found by the Reader.
#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub severity: Severity,
    pub category: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl AuditReport {
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == Severity::Error).count()
    }

    pub fn render(&self) -> String {
        let mut out = format!("═══ Audit: {} ═══\n\n", self.chapter);
        if self.issues.is_empty() {
            out.push_str("✓ 无一致性问题\n");
            return out;
        }
        for issue in &self.issues {
            let icon = match issue.severity {
                Severity::Error => "✗",
                Severity::Warning => "⚠",
                Severity::Info => "ℹ",
            };
            out.push_str(&format!("{icon} [{}] {}\n", issue.category, issue.message));
        }
        out
    }
}

/// Audit extracted effects against the snapshot.
pub fn audit_effects(
    chapter: &str,
    snapshot: &Snapshot,
    effects: &[Op],
    required_effects: &[Op],
) -> AuditReport {
    let mut issues = Vec::new();

    // Check required effects are present
    for req in required_effects {
        if !effects.contains(req) {
            issues.push(AuditIssue {
                severity: Severity::Error,
                category: "required_effect_missing".into(),
                message: format!("必须的 effect 未出现: {}", req.describe()),
            });
        }
    }

    // Check effects reference valid entities
    for effect in effects {
        if let Some(eid) = effect.entity_id() {
            if snapshot.entity(eid).is_none() {
                issues.push(AuditIssue {
                    severity: Severity::Error,
                    category: "invalid_entity".into(),
                    message: format!("Effect 引用了不存在的实体: {eid}"),
                });
            }
        }
    }

    AuditReport {
        chapter: chapter.to_string(),
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::input::entity::Entity;

    fn snap() -> Snapshot {
        Snapshot::from_parts("ch01", vec![
            Entity {
                entity_type: "character".into(), id: "kian".into(), name: None,
                traits: vec![], beliefs: vec![], desires: vec![], intentions: vec![],
                location: None, relationships: vec![], inventory: vec!["刀".into()],
                alignment: None, rivals: vec![], members: vec![], properties: vec![],
                connections: vec![], tags: vec![],
            },
        ], vec![], vec![])
    }

    #[test]
    fn audit_passes_with_valid_effects() {
        let effects = vec![Op::RemoveItem { entity: "kian".into(), item: "刀".into() }];
        let report = audit_effects("ch01", &snap(), &effects, &effects);
        assert!(!report.has_errors());
    }

    #[test]
    fn audit_catches_missing_required() {
        let required = vec![Op::RemoveItem { entity: "kian".into(), item: "刀".into() }];
        let actual = vec![];
        let report = audit_effects("ch01", &snap(), &actual, &required);
        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
    }

    #[test]
    fn audit_catches_invalid_entity() {
        let effects = vec![Op::AddTrait { entity: "ghost".into(), value: "test".into() }];
        let report = audit_effects("ch01", &snap(), &effects, &[]);
        assert!(report.has_errors());
        assert!(report.render().contains("ghost"));
    }
}
