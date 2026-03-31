//! Integration test: Hongmenyan (鸿门宴) v3 — validates reasoning engine
//! against the real historical scenario from 《史记·项羽本纪》.
//!
//! New in v3: 曹无伤 (betrayer), facade (刘邦's deception), strength (power),
//! would_shield / indirect_protector / pressure / torn / must_submit / deceiving.

use ledger::*;
use ledger::input::entity::{IntentTarget, Relationship};
use ledger::input::goal::Goal;

// ══════════════════════════════════════════════════════════════════
// Helper: default relationship (no facade)
// ══════════════════════════════════════════════════════════════════

fn rel(target: &str, role: &str, trust: i8, affinity: i8, respect: i8) -> Relationship {
    Relationship {
        target: target.into(),
        role: role.into(),
        trust, affinity, respect,
        facade_affinity: None,
        facade_respect: None,
    }
}

fn rel_facade(
    target: &str, role: &str,
    trust: i8, affinity: i8, respect: i8,
    facade_aff: i8, facade_resp: i8,
) -> Relationship {
    Relationship {
        target: target.into(),
        role: role.into(),
        trust, affinity, respect,
        facade_affinity: Some(facade_aff),
        facade_respect: Some(facade_resp),
    }
}

fn goal(id: &str, want: &str, problem: Option<&str>, status: GoalStatus) -> Goal {
    Goal {
        id: id.into(),
        want: want.into(),
        problem: problem.map(|s| s.into()),
        solution: None,
        status,
        blocked_by: vec![],
        conflicts_with: vec![],
        side_effects: vec![],
        children: vec![],
    }
}

fn goal_conflict(id: &str, want: &str, problem: Option<&str>, status: GoalStatus, conflicts: &[&str]) -> Goal {
    Goal {
        id: id.into(),
        want: want.into(),
        problem: problem.map(|s| s.into()),
        solution: None,
        status,
        blocked_by: vec![],
        conflicts_with: conflicts.iter().map(|s| s.to_string()).collect(),
        side_effects: vec![],
        children: vec![],
    }
}

// ══════════════════════════════════════════════════════════════════
// World construction
// ══════════════════════════════════════════════════════════════════

fn hongmenyan_genesis() -> Snapshot {
    let entities = vec![
        // ── 刘邦 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "liu_bang".into(),
            name: Some("刘邦".into()),
            traits: vec!["隐忍".into(), "善于用人".into(), "圆滑".into()],
            beliefs: vec!["天下可取".into()],
            desires: vec!["称王关中".into(), "保全性命".into()],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec!["survive".into()],
            location: Some("bashang".into()),
            relationships: vec![
                rel("zhang_liang", "谋士", 3, 3, 2),
                rel("fan_kuai", "部将", 3, 2, 1),
                // 真实: 敌对；表面: "臣与将军戮力而攻秦"
                rel_facade("xiang_yu", "对手", -2, -1, 1, 2, 3),
            ],
            inventory: vec!["白璧一双".into(), "玉斗一双".into()],
            goals: vec![
                goal("survive_feast", "从鸿门宴全身而退", Some("项羽意图杀之"), GoalStatus::Active),
                goal_conflict("claim_guanzhong", "称王关中", Some("项羽兵力远胜"), GoalStatus::Background, &["xiang_yu/dominate"]),
            ],
            tags: vec![],
            description: None,
        }),
        // ── 项羽 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "xiang_yu".into(),
            name: Some("项羽".into()),
            traits: vec!["勇猛".into(), "刚愎自用".into(), "重情义".into()],
            beliefs: vec!["力能扛鼎者当王天下".into()],
            desires: vec!["号令天下".into()],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec![],
            location: Some("hongmen".into()),
            relationships: vec![
                rel("fan_zeng", "亚父", 1, 1, 2),
                rel("xiang_bo", "叔父", 2, 3, 1),
                rel("liu_bang", "对手", -1, 0, 0),
            ],
            inventory: vec![],
            goals: vec![
                goal_conflict("dominate", "号令天下诸侯", None, GoalStatus::Active, &["liu_bang/claim_guanzhong"]),
            ],
            tags: vec![],
            description: None,
        }),
        // ── 范增 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "fan_zeng".into(),
            name: Some("范增".into()),
            traits: vec!["老谋深算".into(), "急躁".into(), "忠诚".into()],
            beliefs: vec!["刘邦必成大患".into()],
            desires: vec!["除去刘邦".into()],
            intentions: vec!["设计在宴上杀刘邦".into()],
            intent_targets: vec![
                IntentTarget { action: "刺杀".into(), target: "liu_bang".into() },
            ],
            desire_tags: vec!["eliminate_liu_bang".into()],
            location: Some("hongmen".into()),
            relationships: vec![
                rel("xiang_yu", "主君", 2, 2, 2),
                rel("xiang_zhuang", "可用之人", 1, 0, 0),
            ],
            inventory: vec!["玉玦".into()],
            goals: vec![
                goal("kill_liu_bang", "在鸿门宴上除去刘邦", Some("项羽犹豫不决"), GoalStatus::Active),
            ],
            tags: vec![],
            description: None,
        }),
        // ── 张良 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "zhang_liang".into(),
            name: Some("张良".into()),
            traits: vec!["足智多谋".into(), "冷静".into(), "忠义".into()],
            beliefs: vec!["以智取胜".into()],
            desires: vec!["保刘邦平安".into()],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec!["protect_liu_bang".into(), "survive".into()],
            location: Some("bashang".into()),
            relationships: vec![
                rel("liu_bang", "主君", 3, 3, 2),
                rel("xiang_bo", "故交", 2, 2, 1),
            ],
            inventory: vec![],
            goals: vec![
                goal("protect_liu_bang", "确保刘邦安全离开鸿门", Some("项羽阵营意图行刺"), GoalStatus::Active),
            ],
            tags: vec![],
            description: None,
        }),
        // ── 樊哙 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "fan_kuai".into(),
            name: Some("樊哙".into()),
            traits: vec!["勇猛".into(), "忠诚".into(), "鲁莽".into()],
            beliefs: vec!["以命护主".into()],
            desires: vec!["保护刘邦".into()],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec!["protect_liu_bang".into(), "survive".into()],
            location: Some("bashang".into()),
            relationships: vec![
                rel("liu_bang", "主君", 3, 3, 3),
            ],
            inventory: vec!["剑".into(), "盾".into()],
            goals: vec![],
            tags: vec![],
            description: None,
        }),
        // ── 项伯 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "xiang_bo".into(),
            name: Some("项伯".into()),
            traits: vec!["重情义".into(), "优柔".into()],
            beliefs: vec!["不可恩将仇报".into()],
            desires: vec!["保张良平安".into()],
            intentions: vec![],
            intent_targets: vec![],
            desire_tags: vec![],
            location: Some("hongmen".into()),
            relationships: vec![
                rel("xiang_yu", "侄", 2, 3, 1),
                rel("zhang_liang", "故交", 2, 3, 1),
                rel("liu_bang", "儿女亲家", 1, 2, 0),
            ],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: None,
        }),
        // ── 项庄 ────────────────────────────────────────────────
        Entity::Character(Character {
            id: "xiang_zhuang".into(),
            name: Some("项庄".into()),
            traits: vec!["武艺高强".into(), "服从".into()],
            beliefs: vec![],
            desires: vec!["完成任务".into()],
            intentions: vec!["舞剑刺杀刘邦".into()],
            intent_targets: vec![
                IntentTarget { action: "刺杀".into(), target: "liu_bang".into() },
            ],
            desire_tags: vec!["eliminate_liu_bang".into()],
            location: Some("hongmen".into()),
            relationships: vec![
                rel("xiang_yu", "主君", 2, 1, 3),
                rel("fan_zeng", "上级", 1, 0, 2),
            ],
            inventory: vec!["剑".into()],
            goals: vec![],
            tags: vec![],
            description: None,
        }),
        // ── 曹无伤 — 整个事件的导火索 ──────────────────────────
        Entity::Character(Character {
            id: "cao_wushang".into(),
            name: Some("曹无伤".into()),
            traits: vec!["投机".into()],
            beliefs: vec!["项羽必胜".into()],
            desires: vec![],
            intentions: vec!["向项羽告密".into()],
            intent_targets: vec![
                IntentTarget { action: "告密".into(), target: "liu_bang".into() },
            ],
            desire_tags: vec![],
            location: Some("bashang".into()),
            relationships: vec![
                // 名义上是刘邦的左司马，但暗中背叛
                rel("liu_bang", "左司马", -2, -2, 0),
                rel("xiang_yu", "暗通", 1, 2, 2),
            ],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: None,
        }),

        // ── Locations ───────────────────────────────────────────
        Entity::Location(Location {
            id: "hongmen".into(),
            name: Some("鸿门".into()),
            properties: vec!["军营".into(), "宴会帐篷".into(), "重兵把守".into()],
            connections: vec!["bashang".into()],
            tags: vec![],
            description: None,
        }),
        Entity::Location(Location {
            id: "bashang".into(),
            name: Some("霸上".into()),
            properties: vec!["军营".into(), "关中入口".into()],
            connections: vec!["hongmen".into()],
            tags: vec![],
            description: None,
        }),

        // ── Factions ────────────────────────────────────────────
        Entity::Faction(Faction {
            id: "chu_army".into(),
            name: Some("楚军".into()),
            alignment: None,
            members: vec!["xiang_yu".into(), "fan_zeng".into(), "xiang_bo".into(), "xiang_zhuang".into()],
            rivals: vec!["han_army".into()],
            leader: Some("xiang_yu".into()),
            strength: Some(400000),
            tags: vec![],
            description: None,
        }),
        Entity::Faction(Faction {
            id: "han_army".into(),
            name: Some("汉军".into()),
            alignment: None,
            // 曹无伤也是汉军——阵营内叛徒
            members: vec!["liu_bang".into(), "zhang_liang".into(), "fan_kuai".into(), "cao_wushang".into()],
            rivals: vec!["chu_army".into()],
            leader: Some("liu_bang".into()),
            strength: Some(100000),
            tags: vec![],
            description: None,
        }),
    ];

    let secrets = vec![
        Secret {
            id: "xiang_bo_leak".into(),
            content: "项伯连夜赴霸上，将项羽欲攻刘邦之事告知张良".into(),
            known_by: vec!["xiang_bo".into(), "zhang_liang".into(), "liu_bang".into()],
            revealed_to_reader: true,
            dramatic_function: Some(ledger::input::secret::DramaticFunction::DramaticIrony),
        },
        Secret {
            id: "fan_zeng_plot".into(),
            content: "范增密谋在宴上刺杀刘邦".into(),
            known_by: vec!["fan_zeng".into(), "xiang_zhuang".into()],
            revealed_to_reader: true,
            dramatic_function: Some(ledger::input::secret::DramaticFunction::Suspense),
        },
        Secret {
            id: "sword_dance".into(),
            content: "项庄舞剑实为刺杀刘邦".into(),
            known_by: vec!["fan_zeng".into(), "xiang_zhuang".into(), "xiang_bo".into(), "zhang_liang".into()],
            revealed_to_reader: true,
            dramatic_function: Some(ledger::input::secret::DramaticFunction::Suspense),
        },
        Secret {
            id: "cao_betrayal".into(),
            content: "曹无伤暗中向项羽告密：沛公欲王关中".into(),
            known_by: vec!["cao_wushang".into()],
            revealed_to_reader: true,
            dramatic_function: Some(ledger::input::secret::DramaticFunction::DramaticIrony),
        },
    ];

    let goal_entities = ledger::input::goal::extract_goal_entities(&entities);
    Snapshot::from_parts("the_feast", entities, secrets, goal_entities)
}

/// Feast scene: everyone moves to hongmen.
fn hongmenyan_feast() -> Snapshot {
    let mut snap = hongmenyan_genesis();
    for entity in &mut snap.entities {
        if let Entity::Character(c) = entity {
            if ["liu_bang", "zhang_liang", "fan_kuai", "cao_wushang"].contains(&c.id.as_str()) {
                c.location = Some("hongmen".into());
            }
        }
    }
    snap
}

// ══════════════════════════════════════════════════════════════════
// Tests — Genesis state (before feast)
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn genesis_fact_generation() {
    let snap = hongmenyan_genesis();
    let facts = ledger::state::fact::collect_facts(&snap);
    let text = facts.to_datalog();

    // Intent targets
    assert!(text.contains("intent_target(fan_zeng, liu_bang)."));
    assert!(text.contains("intent_target(xiang_zhuang, liu_bang)."));
    assert!(text.contains("intent_target(cao_wushang, liu_bang)."));

    // Desire tags
    assert!(text.contains("desire_tag(zhang_liang, protect_liu_bang)."));
    assert!(text.contains("desire_tag(fan_zeng, eliminate_liu_bang)."));

    // Facade
    assert!(text.contains("facade_affinity(liu_bang, xiang_yu, 2)."));
    assert!(text.contains("facade_respect(liu_bang, xiang_yu, 3)."));

    // Leader + Strength
    assert!(text.contains("leader(xiang_yu, chu_army)."));
    assert!(text.contains("leader(liu_bang, han_army)."));
    assert!(text.contains("strength(chu_army, 400000)."));
    assert!(text.contains("strength(han_army, 100000)."));

    // 曹无伤 is han_army member
    assert!(text.contains("member(cao_wushang, han_army)."));
}

#[tokio::test]
async fn genesis_xiang_bo_no_longer_enemy_of_anyone_bonded() {
    let snap = hongmenyan_genesis();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // xiang_bo has personal_bond with zhang_liang (affinity 2) AND liu_bang (affinity 2 now)
    assert!(result.contains("personal_bond", &["xiang_bo", "zhang_liang"]));
    assert!(result.contains("personal_bond", &["xiang_bo", "liu_bang"]));

    // → NOT enemy with either
    assert!(!result.contains("enemy", &["xiang_bo", "zhang_liang"]));
    assert!(!result.contains("enemy", &["zhang_liang", "xiang_bo"]));
    assert!(!result.contains("enemy", &["xiang_bo", "liu_bang"]));
    assert!(!result.contains("enemy", &["liu_bang", "xiang_bo"]));
}

#[tokio::test]
async fn genesis_cao_wushang_betrayal() {
    let snap = hongmenyan_genesis();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // 曹无伤 threatens liu_bang (via intent_target)
    assert!(result.contains("threatens", &["cao_wushang", "liu_bang"]));

    // 曹无伤 is personal_enemy of liu_bang (affinity -2)
    assert!(result.contains("personal_enemy", &["cao_wushang", "liu_bang"]));

    // 曹无伤 has personal_bond with xiang_yu (affinity 2) → not enemy of xiang_yu
    assert!(result.contains("personal_bond", &["cao_wushang", "xiang_yu"]));
    assert!(!result.contains("enemy", &["cao_wushang", "xiang_yu"]));

    // 曹无伤 knows cao_betrayal secret, liu_bang doesn't → betrayal_opportunity
    let betrayals = result.triples("betrayal_opportunity");
    assert!(betrayals.iter().any(|(p, v, s)|
        *p == "cao_wushang" && *v == "liu_bang" && *s == "cao_betrayal"
    ), "曹无伤应该有对刘邦的背叛时机");

    // dramatic_irony: liu_bang doesn't know about cao_betrayal
    assert!(result.contains("dramatic_irony", &["cao_betrayal", "liu_bang"]));
}

#[tokio::test]
async fn genesis_threatens() {
    let snap = hongmenyan_genesis();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    assert!(result.contains("threatens", &["fan_zeng", "liu_bang"]));
    assert!(result.contains("threatens", &["xiang_zhuang", "liu_bang"]));
    assert!(result.contains("threatens", &["cao_wushang", "liu_bang"]));
    // No one threatens xiang_yu
    assert!(!result.contains("threatens", &["liu_bang", "xiang_yu"]));
}

#[tokio::test]
async fn genesis_suspense() {
    let snap = hongmenyan_genesis();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    assert!(result.contains("suspense", &["liu_bang", "survive_feast"]));
    assert!(result.contains("suspense", &["xiang_yu", "dominate"]));
    assert!(result.contains("suspense", &["fan_zeng", "kill_liu_bang"]));
}

#[tokio::test]
async fn genesis_alliance_via_desire_tags() {
    let snap = hongmenyan_genesis();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // protect_liu_bang alliance
    assert!(result.contains("alliance_opportunity", &["zhang_liang", "fan_kuai", "protect_liu_bang"])
         || result.contains("alliance_opportunity", &["fan_kuai", "zhang_liang", "protect_liu_bang"]));

    // eliminate_liu_bang alliance
    assert!(result.contains("alliance_opportunity", &["fan_zeng", "xiang_zhuang", "eliminate_liu_bang"])
         || result.contains("alliance_opportunity", &["xiang_zhuang", "fan_zeng", "eliminate_liu_bang"]));
}

// ══════════════════════════════════════════════════════════════════
// Tests — Feast state (everyone at hongmen)
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn feast_armed_danger() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    assert!(result.contains("armed_danger", &["fan_zeng", "liu_bang"]));
    assert!(result.contains("armed_danger", &["xiang_zhuang", "liu_bang"]));
    assert!(result.contains("armed_danger", &["cao_wushang", "liu_bang"]));
}

#[tokio::test]
async fn feast_protector_direct() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // fan_kuai + zhang_liang: would_sacrifice + armed_danger on liu_bang
    assert!(result.contains("protector", &["fan_kuai", "liu_bang"]));
    assert!(result.contains("protector", &["zhang_liang", "liu_bang"]));
}

#[tokio::test]
async fn feast_would_shield_transitive() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // xiang_bo→zhang_liang: affinity 3, trust 2 → would_sacrifice ✓
    // zhang_liang→liu_bang: affinity 3, trust 3 → would_sacrifice ✓
    // → would_shield(xiang_bo, liu_bang) ← 项伯经由张良保护刘邦
    assert!(result.contains("would_shield", &["xiang_bo", "liu_bang"]));

    // fan_kuai→liu_bang→zhang_liang chain
    assert!(result.contains("would_shield", &["fan_kuai", "zhang_liang"]));
}

#[tokio::test]
async fn feast_xiang_bo_indirect_protector() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // xiang_bo→zhang_liang: affinity 3 → would_sacrifice ✓
    // zhang_liang→liu_bang: affinity 3 → would_sacrifice ✓
    // → would_shield(xiang_bo, liu_bang) ✓
    // + armed_danger on liu_bang → indirect_protector!
    assert!(result.contains("would_shield", &["xiang_bo", "liu_bang"]));
    assert!(result.contains("indirect_protector", &["xiang_bo", "liu_bang"]),
        "项伯应该是刘邦的间接保护者（经由张良）");

    // Also not an enemy
    assert!(!result.contains("enemy", &["xiang_bo", "liu_bang"]));
}

#[tokio::test]
async fn feast_torn_xiang_yu() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // pressure_to_harm(xiang_yu, liu_bang, fan_zeng):
    //   xiang_yu would_obey fan_zeng (respect 2 → ✓)
    //   fan_zeng threatens liu_bang ✓
    //   xiang_yu can_meet liu_bang ✓
    assert!(result.contains("pressure_to_harm", &["xiang_yu", "liu_bang", "fan_zeng"]),
        "范增应该对项羽施加杀刘邦的压力");

    // pressure_to_spare(xiang_yu, liu_bang, xiang_bo):
    //   xiang_yu would_sacrifice xiang_bo (affinity 3, trust 2 → ✓)
    //   xiang_bo would_shield liu_bang (via zhang_liang → ✓)
    assert!(result.contains("pressure_to_spare", &["xiang_yu", "liu_bang", "xiang_bo"]),
        "项伯应该对项羽施加保刘邦的压力");

    // → torn(xiang_yu, liu_bang) = "项王默然不应"
    assert!(result.contains("torn", &["xiang_yu", "liu_bang"]),
        "项羽应该在杀与不杀刘邦之间犹豫");
}

#[tokio::test]
async fn feast_power_advantage() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // 楚军 400000 vs 汉军 100000 → 4:1 ratio >= 2x
    assert!(result.contains("power_advantage", &["chu_army", "han_army"]));
    assert!(!result.contains("power_advantage", &["han_army", "chu_army"]));
}

#[tokio::test]
async fn feast_must_submit() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // must_submit: weaker faction member facing rival leader only
    assert!(result.contains("must_submit", &["liu_bang", "xiang_yu"]));
    assert!(result.contains("must_submit", &["zhang_liang", "xiang_yu"]));
    assert!(result.contains("must_submit", &["fan_kuai", "xiang_yu"]));

    // 反方向不成立
    assert!(!result.contains("must_submit", &["xiang_yu", "liu_bang"]));

    // 不会对非 leader 隐忍
    assert!(!result.contains("must_submit", &["liu_bang", "fan_zeng"]));
    assert!(!result.contains("must_submit", &["liu_bang", "xiang_bo"]));

    // 精确 4 条：汉军 4 人 × 楚军 leader 1 人
    let count = result.get("must_submit").map(|r| r.len()).unwrap_or(0);
    println!("must_submit: {}", count);
    assert_eq!(count, 4, "must_submit should be exactly 4 (han_army members → xiang_yu)");
}

#[tokio::test]
async fn feast_deceiving() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // 刘邦: real affinity -1 to xiang_yu, facade_affinity 2 → deceiving
    assert!(result.contains("deceiving", &["liu_bang", "xiang_yu"]));

    // 项羽 trust for liu_bang is -1 → deceived requires trust >= 0. NOT deceived!
    // Actually: xiang_yu→liu_bang trust is -1, so NOT deceived by rule 1.
    // Rule 2 (fallback): ~trust(xiang_yu, liu_bang, ?Any) — but trust IS defined as -1.
    // So xiang_yu is NOT deceived. Historically accurate? Debatable.
    // In the story, xiang_yu WAS partially convinced by liu_bang's performance.
    // But our data says xiang_yu distrusts liu_bang → not easily deceived. Fair enough.
}

#[tokio::test]
async fn feast_danger_excludes_bonded() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // Real dangers
    assert!(result.contains("danger", &["liu_bang", "xiang_yu"]));
    assert!(result.contains("danger", &["liu_bang", "fan_zeng"]));
    assert!(result.contains("danger", &["liu_bang", "xiang_zhuang"]));

    // xiang_bo NOT danger to liu_bang or zhang_liang (personal_bond)
    assert!(!result.contains("danger", &["liu_bang", "xiang_bo"]));
    assert!(!result.contains("danger", &["zhang_liang", "xiang_bo"]));
    assert!(!result.contains("danger", &["xiang_bo", "zhang_liang"]));
}

#[tokio::test]
async fn feast_dramatic_irony() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    // 曹无伤的背叛：刘邦不知道（直到宴后项羽说漏嘴）
    assert!(result.contains("dramatic_irony", &["cao_betrayal", "liu_bang"]));
    assert!(result.contains("dramatic_irony", &["cao_betrayal", "zhang_liang"]));

    // 范增的计划：项羽不知道
    assert!(result.contains("dramatic_irony", &["fan_zeng_plot", "xiang_yu"]));

    // 项伯泄密：范增不知道
    assert!(result.contains("dramatic_irony", &["xiang_bo_leak", "fan_zeng"]));
}

#[tokio::test]
async fn feast_critical_reveals() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    let critical = result.triples("critical_reveal");
    println!("critical_reveal: {:?}", critical);

    // 张良警告刘邦（trust-based, not enemy）
    assert!(critical.iter().any(|(s, i, u)|
        *s == "sword_dance" && *i == "zhang_liang" && *u == "liu_bang"
    ), "张良应该警告刘邦项庄舞剑的意图");

    // 范增向项羽汇报（would_obey）
    assert!(critical.iter().any(|(s, i, u)|
        *s == "fan_zeng_plot" && *i == "fan_zeng" && *u == "xiang_yu"
    ), "范增应该向项羽汇报刺杀计划");

    // 范增不会向刘邦透露计划（enemy 排除）
    assert!(!critical.iter().any(|(s, i, u)|
        *s == "fan_zeng_plot" && *i == "fan_zeng" && *u == "liu_bang"
    ), "范增不应该向刘邦透露刺杀计划");

    // 项庄不会向刘邦透露（enemy 排除）
    assert!(!critical.iter().any(|(s, i, u)|
        *s == "sword_dance" && *i == "xiang_zhuang" && *u == "liu_bang"
    ), "项庄不应该向刘邦透露舞剑意图");

    // Much fewer than possible_reveal
    let possible_count = result.get("possible_reveal").map(|r| r.len()).unwrap_or(0);
    println!("critical_reveal: {} vs possible_reveal: {}", critical.len(), possible_count);
    assert!(critical.len() < possible_count / 2, "critical should be much fewer than possible");
}

// ══════════════════════════════════════════════════════════════════
// Summary test — full v3 output
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn feast_full_reasoning_v3() {
    let snap = hongmenyan_feast();
    let result = Program::from_snapshot(&snap, None).run().await.unwrap();

    println!("\n{}", "=".repeat(60));
    println!("鸿门宴 v3 — 推理引擎完整输出");
    println!("{}\n", "=".repeat(60));

    let predicates = [
        "active_danger", "armed_danger", "threatens", "plots_against",
        "protector", "indirect_protector", "would_shield",
        "pressure_to_harm", "pressure_to_spare", "torn",
        "power_advantage", "must_submit",
        "deceiving", "deceived",
        "betrayal_opportunity", "critical_reveal",
        "dramatic_irony", "alliance_opportunity", "suspense",
        "would_confide", "would_obey", "would_sacrifice",
        "enemy", "personal_bond", "personal_enemy",
        "info_cascade",
    ];

    for pred in predicates {
        if let Some(rows) = result.get(pred) {
            println!("[{pred}] ({} facts)", rows.len());
            for row in rows {
                println!("  {}", row.join(", "));
            }
            println!();
        }
    }

    println!("Total derived facts: {}", result.total_facts);

    // ── Key narrative assertions ────────────────────────────────
    // 三条刺杀链
    assert!(result.contains("armed_danger", &["fan_zeng", "liu_bang"]));
    assert!(result.contains("armed_danger", &["xiang_zhuang", "liu_bang"]));
    assert!(result.contains("armed_danger", &["cao_wushang", "liu_bang"]));

    // 两个直接守护者 + 一个间接守护者
    assert!(result.contains("protector", &["fan_kuai", "liu_bang"]));
    assert!(result.contains("protector", &["zhang_liang", "liu_bang"]));
    assert!(result.contains("indirect_protector", &["xiang_bo", "liu_bang"]),
        "项伯应是刘邦的间接保护者");

    // 项羽犹豫
    assert!(result.contains("torn", &["xiang_yu", "liu_bang"]),
        "项羽应该犹豫不决");

    // 项伯不是敌人
    assert!(!result.contains("enemy", &["xiang_bo", "liu_bang"]));
    assert!(!result.contains("enemy", &["xiang_bo", "zhang_liang"]));

    // 力量碾压
    assert!(result.contains("power_advantage", &["chu_army", "han_army"]));
    assert!(result.contains("must_submit", &["liu_bang", "xiang_yu"]));

    // 刘邦在演戏
    assert!(result.contains("deceiving", &["liu_bang", "xiang_yu"]));

    // 曹无伤的背叛
    assert!(result.contains("threatens", &["cao_wushang", "liu_bang"]));

    // 核心悬念
    assert!(result.contains("suspense", &["liu_bang", "survive_feast"]));
}

// ══════════════════════════════════════════════════════════════════
// Snapshot Time Travel — reasoning diff across beats
// ══════════════════════════════════════════════════════════════════

/// Apply effects to a snapshot in place (simulate beat effects).
fn apply_reveals(snap: &mut Snapshot, secret_id: &str, to: &[&str]) {
    for secret in &mut snap.secrets {
        if secret.id == secret_id {
            for char_id in to {
                if !secret.known_by.contains(&char_id.to_string()) {
                    secret.known_by.push(char_id.to_string());
                }
            }
        }
    }
}

fn move_chars(snap: &mut Snapshot, chars: &[&str], location: &str) {
    for entity in &mut snap.entities {
        if let Entity::Character(c) = entity {
            if chars.contains(&c.id.as_str()) {
                c.location = Some(location.into());
            }
        }
    }
}

#[tokio::test]
async fn time_travel_three_beats() {
    // ── Beat 0: Genesis — two camps, tension building ───────────
    let snap0 = hongmenyan_genesis();
    let r0 = Program::from_snapshot(&snap0, None).run().await.unwrap();

    // ── Beat 1: 刘邦赴宴 — everyone at hongmen ─────────────────
    let mut snap1 = snap0.clone();
    move_chars(&mut snap1, &["liu_bang", "zhang_liang", "fan_kuai", "cao_wushang"], "hongmen");
    let r1 = Program::from_snapshot(&snap1, None).run().await.unwrap();

    // ── Beat 2: 张良察觉舞剑，警告刘邦 ─────────────────────────
    let mut snap2 = snap1.clone();
    apply_reveals(&mut snap2, "sword_dance", &["liu_bang"]);
    let r2 = Program::from_snapshot(&snap2, None).run().await.unwrap();

    // ═══ Diff: Beat 0 → Beat 1 (刘邦赴宴) ═══════════════════════
    let diff01 = ReasoningDiff::diff(&r0, &r1);

    println!("\n{}", "=".repeat(60));
    println!("Beat 0 → Beat 1: 刘邦率众赴鸿门");
    println!("{}", "=".repeat(60));

    let high_signal = [
        "active_danger", "armed_danger", "protector", "indirect_protector",
        "torn", "must_submit", "deceiving", "deceived",
        "pressure_to_harm", "pressure_to_spare",
        "critical_reveal", "betrayal_opportunity",
        "dramatic_irony", "suspense",
    ];

    println!("\n涌现 (+):");
    for pred in &high_signal {
        let facts = diff01.emerged_for(pred);
        if !facts.is_empty() {
            for row in facts {
                println!("  + {}({})", pred, row.join(", "));
            }
        }
    }
    println!("\n消解 (-):");
    for pred in &high_signal {
        let facts = diff01.resolved_for(pred);
        if !facts.is_empty() {
            for row in facts {
                println!("  - {}({})", pred, row.join(", "));
            }
        }
    }

    // Key assertions: what EMERGED when liu_bang walked into hongmen
    assert!(!diff01.emerged_for("torn").is_empty(),
        "项羽的犹豫应该在刘邦赴宴时涌现");
    assert!(!diff01.emerged_for("armed_danger").is_empty(),
        "范增/项庄的定向威胁应该在见面时激活");
    assert!(!diff01.emerged_for("must_submit").is_empty(),
        "力量碾压应该在见面时生效");
    assert!(!diff01.emerged_for("indirect_protector").is_empty(),
        "项伯的间接保护应该在所有人到场时激活");
    assert!(!diff01.emerged_for("pressure_to_harm").is_empty(),
        "范增对项羽的施压应该在见面时激活");
    assert!(!diff01.emerged_for("pressure_to_spare").is_empty(),
        "项伯对项羽的求情应该在见面时激活");

    // protector/deceiving may already exist in beat 0 (cao_wushang at bashang)
    // so they might not appear in the diff — that's correct

    // ═══ Diff: Beat 1 → Beat 2 (张良警告刘邦) ═══════════════════
    let diff12 = ReasoningDiff::diff(&r1, &r2);

    println!("\n{}", "=".repeat(60));
    println!("Beat 1 → Beat 2: 张良警告刘邦「项庄舞剑，意在沛公」");
    println!("{}", "=".repeat(60));

    println!("\n涌现 (+):");
    for pred in &high_signal {
        let facts = diff12.emerged_for(pred);
        if !facts.is_empty() {
            for row in facts {
                println!("  + {}({})", pred, row.join(", "));
            }
        }
    }
    println!("\n消解 (-):");
    for pred in &high_signal {
        let facts = diff12.resolved_for(pred);
        if !facts.is_empty() {
            for row in facts {
                println!("  - {}({})", pred, row.join(", "));
            }
        }
    }

    // Key assertion: dramatic_irony for sword_dance on liu_bang should RESOLVE
    assert!(diff12.resolved_for("dramatic_irony").iter().any(|row|
        row.len() >= 2 && row[0] == "sword_dance" && row[1] == "liu_bang"
    ), "刘邦得知舞剑秘密后，dramatic_irony 应该消解");

    // Some critical_reveals should disappear (liu_bang already knows sword_dance)
    assert!(!diff12.resolved_for("critical_reveal").is_empty(),
        "刘邦已知舞剑秘密，部分 critical_reveal 应该消失");

    // Total change summary
    println!("\n{}", "=".repeat(60));
    println!("变化汇总:");
    println!("  Beat 0→1: +{} emerged, -{} resolved", diff01.emerged_count(), diff01.resolved_count());
    println!("  Beat 1→2: +{} emerged, -{} resolved", diff12.emerged_count(), diff12.resolved_count());
    println!("{}", "=".repeat(60));
}
