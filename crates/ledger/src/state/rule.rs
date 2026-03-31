//! Rule management — builtin + user-extensible Datalog rules.
//!
//! `RuleSet` collects rules from builtin sources and user `.dl` files.
//! Each rule entry carries its export predicates, so the export list
//! is always in sync with the actual rules.

use std::path::Path;

// ══════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════

/// Where a rule came from.
#[derive(Debug, Clone)]
pub enum RuleSource {
    Builtin(&'static str),
    UserFile(String),
}

/// A single rule entry: Datalog text + the predicates it exports.
#[derive(Debug, Clone)]
pub struct RuleEntry {
    pub source: RuleSource,
    pub text: String,
    pub exports: Vec<String>,
}

/// Collection of all rules for a reasoning session.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    entries: Vec<RuleEntry>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule entry.
    pub fn add(&mut self, entry: RuleEntry) {
        self.entries.push(entry);
    }

    /// Merge another RuleSet into this one.
    pub fn extend(&mut self, other: RuleSet) {
        self.entries.extend(other.entries);
    }

    /// All export predicate names, deduplicated and sorted.
    pub fn exports(&self) -> Vec<String> {
        let mut exports: Vec<String> = self
            .entries
            .iter()
            .flat_map(|e| e.exports.iter().cloned())
            .collect();
        exports.sort();
        exports.dedup();
        exports
    }

    /// Serialize all rules to a Datalog program fragment.
    pub fn to_datalog(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            match &entry.source {
                RuleSource::Builtin(name) => {
                    out.push_str(&format!("% --- rules: {name} ---\n"));
                }
                RuleSource::UserFile(path) => {
                    out.push_str(&format!("% --- rules: {path} ---\n"));
                }
            }
            out.push_str(&entry.text);
            out.push('\n');
        }
        out
    }

    /// Serialize export declarations.
    pub fn export_declarations(&self) -> String {
        let mut out = String::from("% === Exports ===\n");
        for pred in &self.exports() {
            out.push_str(&format!("@export {pred} :- csv{{}}.\n"));
        }
        out
    }

    /// Load all builtin rules.
    pub fn builtins() -> Self {
        let mut rs = Self::new();

        rs.add(RuleEntry {
            source: RuleSource::Builtin("entity"),
            text: ENTITY_RULES.to_string(),
            exports: vec![
                "active_danger".into(),
                "armed_danger".into(),
                "can_meet".into(),
                "danger".into(),
                "enemy".into(),
                "heard_of".into(),
                "personal_bond".into(),
                "personal_enemy".into(),
                "protector".into(),
                "reachable".into(),
                "rebellion_seed".into(),
                "would_confide".into(),
                "would_obey".into(),
                "would_sacrifice".into(),
            ],
        });

        rs.add(RuleEntry {
            source: RuleSource::Builtin("social"),
            text: SOCIAL_RULES.to_string(),
            exports: vec![
                "would_shield".into(),
                "indirect_protector".into(),
                "pressure_to_harm".into(),
                "pressure_to_spare".into(),
                "torn".into(),
                "power_advantage".into(),
                "must_submit".into(),
                "deceiving".into(),
                "deceived".into(),
            ],
        });

        rs.add(RuleEntry {
            source: RuleSource::Builtin("goal"),
            text: GOAL_RULES.to_string(),
            exports: vec![
                "suspense".into(),
                "active_conflict".into(),
                "unblocked".into(),
                "would_unblock".into(),
            ],
        });

        rs.add(RuleEntry {
            source: RuleSource::Builtin("intent"),
            text: INTENT_RULES.to_string(),
            exports: vec![
                "threatens".into(),
                "plots_against".into(),
            ],
        });

        rs.add(RuleEntry {
            source: RuleSource::Builtin("secret"),
            text: SECRET_RULES.to_string(),
            exports: vec![
                "dramatic_irony".into(),
            ],
        });

        rs.add(RuleEntry {
            source: RuleSource::Builtin("narrative"),
            text: NARRATIVE_RULES.to_string(),
            exports: vec![
                "betrayal_opportunity".into(),
                "critical_reveal".into(),
                "possible_reveal".into(),
                "info_cascade".into(),
                "alliance_opportunity".into(),
                "goal_conflict_encounter".into(),
                "orphaned_secret".into(),
            ],
        });

        rs
    }

    /// Load user rules from `cards/rules/*.dl` files.
    pub fn load_user(cards_dir: &Path) -> Self {
        let mut rs = Self::new();
        let rules_dir = cards_dir.join("rules");
        if !rules_dir.exists() {
            return rs;
        }

        let Ok(entries) = std::fs::read_dir(&rules_dir) else {
            return rs;
        };

        let mut paths: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "dl"))
            .collect();
        paths.sort();

        for path in paths {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };

            // Extract exports from `% @export predicate_name` comments
            let exports: Vec<String> = text
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    trimmed
                        .strip_prefix("% @export ")
                        .or_else(|| trimmed.strip_prefix("%@export "))
                        .map(|s| s.trim().to_string())
                })
                .collect();

            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            rs.add(RuleEntry {
                source: RuleSource::UserFile(filename),
                text,
                exports,
            });
        }

        rs
    }
}

// ══════════════════════════════════════════════════════════════════
// Builtin rule texts
// ══════════════════════════════════════════════════════════════════

const ENTITY_RULES: &str = r#"% Social: who can meet (same location)
can_meet(?A, ?B) :- at(?A, ?P), at(?B, ?P), ?A != ?B, character(?A), character(?B).

% Helper: personal bond (affinity >= 2, either direction) overrides faction rivalry
personal_bond(?A, ?B) :- affinity(?A, ?B, ?V), ?V >= 2.

% Social: enemy detection (rival factions, unless personal bond overrides)
enemy(?A, ?B) :- member(?A, ?S1), member(?B, ?S2), rival(?S1, ?S2), ?A != ?B, ~personal_bond(?A, ?B), ~personal_bond(?B, ?A).

% Social: personal enemy (affinity <= -2)
personal_enemy(?A, ?B) :- affinity(?A, ?B, ?V), ?V <= -2, character(?A), character(?B).

% Danger (background): faction enemies meeting — low signal, kept for completeness
danger(?A, ?B) :- can_meet(?A, ?B), enemy(?A, ?B).
danger(?A, ?B) :- can_meet(?A, ?B), personal_enemy(?A, ?B).

% Armed danger: targeted threat (intent to harm + can meet)
armed_danger(?Attacker, ?Target) :- threatens(?Attacker, ?Target), can_meet(?Attacker, ?Target).

% Active danger: someone with specific hostile intent or personal hatred faces you
active_danger(?A, ?B) :- armed_danger(?A, ?B).
active_danger(?A, ?B) :- can_meet(?A, ?B), personal_enemy(?A, ?B).

% Protection: would sacrifice + someone has armed_danger on the protectee
protector(?Guardian, ?Protectee) :- would_sacrifice(?Guardian, ?Protectee), armed_danger(?Attacker, ?Protectee), ?Guardian != ?Attacker.

% Location: reachable (transitive connections)
reachable(?A, ?B) :- connected(?A, ?B).
reachable(?A, ?C) :- connected(?A, ?B), reachable(?B, ?C), ?A != ?C.

% Social: heard-of via relationships
heard_of(?A, ?C) :- role(?A, ?B, ?R1), role(?B, ?C, ?R2), ?A != ?C.

% Trust: would confide a secret (trust >= 2 + can meet)
would_confide(?A, ?B) :- trust(?A, ?B, ?T), ?T >= 2, can_meet(?A, ?B).

% Trust: would obey (respect >= 2)
would_obey(?A, ?B) :- respect(?A, ?B, ?R), ?R >= 2.

% Trust: would sacrifice (affinity >= 3 + trust >= 1)
would_sacrifice(?A, ?B) :- affinity(?A, ?B, ?Af), ?Af >= 3, trust(?A, ?B, ?T), ?T >= 1.

% Conflict: rebellion seed (low respect + low affinity)
rebellion_seed(?Sub, ?Sup) :- respect(?Sub, ?Sup, ?R), ?R <= -1, affinity(?Sub, ?Sup, ?A), ?A <= -1.
"#;

const GOAL_RULES: &str = r#"% Suspense: active goal with no solution
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
"#;

const SOCIAL_RULES: &str = r#"% Indirect protection: A would sacrifice for C, C would sacrifice for B → A shields B
would_shield(?A, ?B) :-
  would_sacrifice(?A, ?C),
  would_sacrifice(?C, ?B),
  ?A != ?B.

% Indirect protector: would_shield + target has armed_danger + can meet
indirect_protector(?A, ?B) :-
  would_shield(?A, ?B),
  armed_danger(?Attacker, ?B),
  can_meet(?A, ?B),
  ?A != ?Attacker.

% Pressure to harm: A obeys instigator, instigator threatens B, A can meet B
pressure_to_harm(?A, ?B, ?Instigator) :-
  would_obey(?A, ?Instigator),
  threatens(?Instigator, ?B),
  can_meet(?A, ?B),
  ?A != ?B, ?A != ?Instigator.

% Pressure to spare: A would sacrifice for pleader, pleader shields B
pressure_to_spare(?A, ?B, ?Pleader) :-
  would_sacrifice(?A, ?Pleader),
  would_shield(?Pleader, ?B),
  can_meet(?A, ?B),
  ?A != ?B, ?A != ?Pleader.

% Torn: simultaneous pressure to harm and to spare the same person
torn(?A, ?B) :-
  pressure_to_harm(?A, ?B, ?X),
  pressure_to_spare(?A, ?B, ?Y),
  ?X != ?Y.

% Power advantage: faction strength >= 2x opponent
power_advantage(?StrongF, ?WeakF) :-
  strength(?StrongF, ?S1),
  strength(?WeakF, ?S2),
  rival(?StrongF, ?WeakF),
  ?S1 >= ?S2 * 2.

% Must submit: weaker faction member facing the rival faction's leader
must_submit(?A, ?B) :-
  member(?A, ?WeakF),
  leader(?B, ?StrongF),
  power_advantage(?StrongF, ?WeakF),
  can_meet(?A, ?B).

% Deception: real affinity negative + facade affinity positive
deceiving(?A, ?B) :-
  affinity(?A, ?B, ?Real), ?Real <= 0,
  facade_affinity(?A, ?B, ?Fake), ?Fake >= 2.

% Deceived: someone is deceiving you + you don't distrust them
deceived(?B, ?A) :-
  deceiving(?A, ?B),
  trust(?B, ?A, ?T), ?T >= 0.

% Deceived (fallback: no trust data → assumed deceived)
deceived(?B, ?A) :-
  deceiving(?A, ?B),
  ~trust(?B, ?A, ?Any).
"#;

const INTENT_RULES: &str = r#"% Threat: intent_target fact (emitted by fact.rs when intent references an entity)
threatens(?A, ?B) :- intent_target(?A, ?B), character(?A), character(?B).

% Plots against: has a secret weapon against someone + threatens them
plots_against(?A, ?B) :- threatens(?A, ?B), secret_known_by(?S, ?A), ~secret_known_by(?S, ?B).
"#;

const SECRET_RULES: &str = r#"% Dramatic irony: reader and some char know, but another char doesn't
dramatic_irony(?Secret, ?Uninformed) :-
  secret_known_by(?Secret, ?Informed),
  secret_revealed_to_reader(?Secret),
  character(?Uninformed),
  ?Informed != ?Uninformed,
  ~secret_known_by(?Secret, ?Uninformed).
"#;

const NARRATIVE_RULES: &str = r#"% 背叛时机（强）：持有秘密 + 对受害者有敌意(affinity <= -1) + 能见面
betrayal_opportunity(?Plotter, ?Victim, ?Secret) :-
  secret_known_by(?Secret, ?Plotter),
  ~secret_known_by(?Secret, ?Victim),
  affinity(?Plotter, ?Victim, ?Af), ?Af <= -1,
  can_meet(?Plotter, ?Victim),
  character(?Victim).

% 背叛时机（信任型）：受害者信任施害者(trust >= 1) + 施害者不信任受害者(trust <= 0) + 能见面
betrayal_opportunity(?Plotter, ?Victim, ?Secret) :-
  secret_known_by(?Secret, ?Plotter),
  ~secret_known_by(?Secret, ?Victim),
  trust(?Victim, ?Plotter, ?VT), ?VT >= 1,
  trust(?Plotter, ?Victim, ?PT), ?PT <= 0,
  can_meet(?Plotter, ?Victim),
  character(?Victim).

% 揭秘时机（完整）：知情者与不知情者在同一地点
possible_reveal(?Secret, ?Informed, ?Uninformed) :-
  secret_known_by(?Secret, ?Informed),
  ~secret_known_by(?Secret, ?Uninformed),
  can_meet(?Informed, ?Uninformed),
  character(?Uninformed).

% 关键揭秘：对受威胁者的预警（知情者不是敌人才会告知）
critical_reveal(?Secret, ?Informed, ?Target) :-
  possible_reveal(?Secret, ?Informed, ?Target),
  plots_against(?Plotter, ?Target),
  secret_known_by(?Secret, ?Plotter),
  ~enemy(?Informed, ?Target).

% 关键揭秘：对同盟的预警（知情者信任不知情者）
critical_reveal(?Secret, ?Informed, ?Ally) :-
  possible_reveal(?Secret, ?Informed, ?Ally),
  trust(?Informed, ?Ally, ?T), ?T >= 1,
  ~enemy(?Informed, ?Ally).

% 关键揭秘：向上级汇报（不知情者是知情者 would_obey 的对象）
critical_reveal(?Secret, ?Informed, ?Superior) :-
  possible_reveal(?Secret, ?Informed, ?Superior),
  would_obey(?Informed, ?Superior).

% 信息级联：知情者能传给谁（信任 >= 1）
info_cascade(?Secret, ?Bridge, ?Target) :-
  secret_known_by(?Secret, ?Bridge),
  can_meet(?Bridge, ?Target),
  trust(?Bridge, ?Target, ?T), ?T >= 1,
  ~secret_known_by(?Secret, ?Target),
  character(?Target).

% 联盟机会：共同欲望 + 正向亲和
alliance_opportunity(?A, ?B, ?Want) :-
  desires(?A, ?Want), desires(?B, ?Want),
  affinity(?A, ?B, ?V), ?V >= 0,
  character(?A), character(?B), ?A != ?B.

% 联盟机会（回退：无亲和数据时）
alliance_opportunity(?A, ?B, ?Want) :-
  desires(?A, ?Want), desires(?B, ?Want),
  character(?A), character(?B), ?A != ?B,
  ~affinity(?A, ?B, ?Any).

% 联盟机会：共同 desire_tag
alliance_opportunity(?A, ?B, ?Tag) :-
  desire_tag(?A, ?Tag), desire_tag(?B, ?Tag),
  character(?A), character(?B), ?A != ?B,
  affinity(?A, ?B, ?V), ?V >= 0.

% 联盟机会（回退：desire_tag 无亲和数据时）
alliance_opportunity(?A, ?B, ?Tag) :-
  desire_tag(?A, ?Tag), desire_tag(?B, ?Tag),
  character(?A), character(?B), ?A != ?B,
  ~affinity(?A, ?B, ?Any).

% 目标冲突相遇：冲突目标 + 能见面
goal_conflict_encounter(?OA, ?GA, ?OB, ?GB) :-
  active_conflict(?OA, ?GA, ?OB, ?GB),
  can_meet(?OA, ?OB).

% 孤立秘密：读者知道但没有角色知道
has_knower(?S) :- secret_known_by(?S, ?C).
orphaned_secret(?S) :-
  secret(?S),
  secret_revealed_to_reader(?S),
  ~has_knower(?S).
"#;

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_exports() {
        let rs = RuleSet::builtins();
        let exports = rs.exports();
        assert!(exports.contains(&"can_meet".to_string()));
        assert!(exports.contains(&"betrayal_opportunity".to_string()));
        assert!(exports.contains(&"dramatic_irony".to_string()));
        assert!(exports.contains(&"suspense".to_string()));
    }

    #[test]
    fn exports_are_sorted_and_deduped() {
        let rs = RuleSet::builtins();
        let exports = rs.exports();
        for w in exports.windows(2) {
            assert!(w[0] < w[1], "exports not sorted: {} >= {}", w[0], w[1]);
        }
    }

    #[test]
    fn to_datalog_includes_all_rules() {
        let rs = RuleSet::builtins();
        let text = rs.to_datalog();
        assert!(text.contains("can_meet"));
        assert!(text.contains("suspense"));
        assert!(text.contains("dramatic_irony"));
        assert!(text.contains("betrayal_opportunity"));
    }

    #[test]
    fn export_declarations_format() {
        let rs = RuleSet::builtins();
        let decls = rs.export_declarations();
        assert!(decls.contains("@export can_meet :- csv{}."));
        assert!(decls.contains("@export suspense :- csv{}."));
    }

    #[test]
    fn load_user_missing_dir() {
        let rs = RuleSet::load_user(Path::new("/nonexistent"));
        assert!(rs.exports().is_empty());
    }

    #[test]
    fn load_user_with_dl_file() {
        let tmp = tempfile::tempdir().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();
        std::fs::write(
            rules_dir.join("custom.dl"),
            "% @export master_student\nmaster_student(?A, ?B) :- role(?A, ?B, \"师徒\").\n",
        )
        .unwrap();

        let rs = RuleSet::load_user(tmp.path());
        assert!(rs.exports().contains(&"master_student".to_string()));
        assert!(rs.to_datalog().contains("master_student(?A, ?B)"));
    }

    #[test]
    fn load_user_multiple_exports() {
        let tmp = tempfile::tempdir().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();
        std::fs::write(
            rules_dir.join("multi.dl"),
            "% @export pred_a\n% @export pred_b\npred_a(?X) :- character(?X).\npred_b(?X) :- location(?X).\n",
        )
        .unwrap();

        let rs = RuleSet::load_user(tmp.path());
        let exports = rs.exports();
        assert!(exports.contains(&"pred_a".to_string()));
        assert!(exports.contains(&"pred_b".to_string()));
    }

    #[test]
    fn load_user_ignores_non_dl_files() {
        let tmp = tempfile::tempdir().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("readme.txt"), "not a rule file").unwrap();
        std::fs::write(rules_dir.join("notes.md"), "# notes").unwrap();

        let rs = RuleSet::load_user(tmp.path());
        assert!(rs.exports().is_empty());
    }

    #[test]
    fn extend_merges_rulesets() {
        let mut a = RuleSet::new();
        a.add(RuleEntry {
            source: RuleSource::Builtin("test"),
            text: "pred_a(?X) :- character(?X).".into(),
            exports: vec!["pred_a".into()],
        });
        let mut b = RuleSet::new();
        b.add(RuleEntry {
            source: RuleSource::Builtin("test2"),
            text: "pred_b(?X) :- location(?X).".into(),
            exports: vec!["pred_b".into()],
        });
        a.extend(b);
        let exports = a.exports();
        assert!(exports.contains(&"pred_a".to_string()));
        assert!(exports.contains(&"pred_b".to_string()));
    }
}
