//! `elore suggest`
//!
//! Runs the Datalog reasoning engine on the current snapshot to derive
//! narrative possibilities, then displays constraint suggestions.

use std::collections::BTreeMap;

use colored::Colorize;
use std::path::Path;

use ledger::state::content::{Content, ContentTree};

pub async fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let cards_dir = project.join("cards");
    let entities_dir = everlore.join("entities");

    let tree = ContentTree::load(&everlore);

    // Determine which content node to analyze
    let content_id = if let Some(ref active) = tree.active {
        active.clone()
    } else if let Some(ref root) = tree.root {
        // No active node — use the last committed node in DFS order,
        // or root if nothing is committed
        let dfs = tree.dfs_order();
        dfs.iter()
            .rev()
            .find(|id| {
                tree.nodes
                    .get(id.as_str())
                    .is_some_and(|e| e.status == ledger::ContentStatus::Committed)
            })
            .cloned()
            .unwrap_or_else(|| root.clone())
    } else {
        println!(
            "{}",
            "(没有内容树 — 请先在 cards/content/ 创建节点后运行 `elore build`)"
                .yellow()
        );
        return Ok(());
    };

    // Load content cards and build snapshot
    let contents = ledger::card::load_content_cards(&cards_dir)?;
    let content_map: BTreeMap<String, Content> =
        contents.into_iter().map(|c| (c.id.clone(), c)).collect();

    let snapshot = ledger::Snapshot::at_content(
        &content_id,
        &tree,
        &content_map,
        &entities_dir,
        &cards_dir,
        &everlore,
    )?;

    // ── Collect effects for EMA reasoning ───────────────────────
    let pending_effects = content_map
        .get(&content_id)
        .map(|c| c.effects.as_slice())
        .unwrap_or(&[]);

    let history_effects: Vec<(String, Vec<ledger::Op>)> = {
        let dfs = tree.dfs_order();
        let mut hist = Vec::new();
        for nid in &dfs {
            if nid == &content_id {
                break;
            }
            if tree.nodes.get(nid.as_str()).is_some_and(|e| {
                e.status == ledger::ContentStatus::Committed
            }) {
                if let Some(c) = content_map.get(nid) {
                    if !c.effects.is_empty() && tree.is_leaf(nid) {
                        hist.push((nid.clone(), c.effects.clone()));
                    }
                }
            }
        }
        hist
    };

    // ── Run reasoning engine ────────────────────────────────────
    let reasoning = ledger::Program::from_snapshot_with_effects(
        &snapshot,
        pending_effects,
        &history_effects,
        Some(&cards_dir),
    )
        .run()
        .await?;

    // Resolve main_role
    let main_role = ledger::effective_main_role(&content_id, &tree, &content_map);

    println!("{}", "═══ 🔮 叙事推理引擎 ═══".magenta().bold());
    if let Some(ref role) = main_role {
        println!(
            "content: {}  |  main_role: {}  |  推导事实: {}",
            content_id.cyan(),
            role.yellow(),
            reasoning.total_facts
        );
    } else {
        println!(
            "content: {}  |  推导事实: {}",
            content_id.cyan(),
            reasoning.total_facts
        );
    }
    println!();

    let mut has_possibilities = false;

    // Narrative possibilities from reasoning
    let predicates: &[(&str, &str, fn(&[String]) -> String)] = &[
        ("can_meet", "📍 可能的相遇", |row: &[String]| {
            format!("  {} ↔ {}", row[0], row[1])
        }),
        (
            "active_danger",
            "⚔ 危险遭遇（有主动敌意的角色相遇）",
            |row| format!("  {} → {}", row[0], row[1]),
        ),
        (
            "armed_danger",
            "🔪 定向威胁（有明确攻击意图 + 能见面）",
            |row| format!("  {} → {}", row[0], row[1]),
        ),
        (
            "protector",
            "🛡 守护者（愿意牺牲 + 保护对象正受威胁）",
            |row| format!("  {} 守护 {}", row[0], row[1]),
        ),
        (
            "betrayal_opportunity",
            "🗡 背叛时机（持有秘密 + 能见面）",
            |row| {
                format!(
                    "  {} 可对 {} 发动（秘密: {}）",
                    row[0],
                    row[1],
                    row.get(2).map(|s| s.as_str()).unwrap_or("?")
                )
            },
        ),
        (
            "critical_reveal",
            "💬 关键揭秘（高优先级信息传递）",
            |row| {
                format!(
                    "  {} 可向 {} 透露「{}」",
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?"),
                    row[0]
                )
            },
        ),
        ("info_cascade", "🔗 信息级联（秘密传播路径）", |row| {
            format!(
                "  「{}」: {} → {}",
                row[0],
                row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                row.get(2).map(|s| s.as_str()).unwrap_or("?")
            )
        }),
        (
            "dramatic_irony",
            "🎭 戏剧性反讽（读者知道但角色不知道）",
            |row| {
                format!(
                    "  {} 对「{}」一无所知",
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row[0]
                )
            },
        ),
        (
            "alliance_opportunity",
            "🤝 联盟机会（共同欲望）",
            |row| {
                format!(
                    "  {} & {} — 共同渴望: {}",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?")
                )
            },
        ),
        ("suspense", "❓ 悬念（活跃目标无解法）", |row| {
            format!(
                "  {}/{}",
                row[0],
                row.get(1).map(|s| s.as_str()).unwrap_or("?")
            )
        }),
        (
            "goal_conflict_encounter",
            "💥 目标冲突相遇（冲突双方能见面）",
            |row| {
                format!(
                    "  {}/{} ↔ {}/{}",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(3).map(|s| s.as_str()).unwrap_or("?")
                )
            },
        ),
        (
            "orphaned_secret",
            "⚠ 孤立秘密（读者知道但无角色知晓）",
            |row| {
                format!(
                    "  「{}」→ 需要安排角色发现此秘密",
                    row[0]
                )
            },
        ),
        // ── EMA appraisal predicates ────────────────────────────
        (
            "ema_distress",
            "😰 情感压力（事件威胁角色目标）",
            |row| {
                format!(
                    "  {} 因「{}」承受压力",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "ema_fear",
            "😨 恐惧/担忧（预感不利事件）",
            |row| {
                format!(
                    "  {} 担忧「{}」",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "ema_anger",
            "😠 愤怒/怨恨（归咎于某人）",
            |row| {
                format!(
                    "  {} 对 {} 因「{}」",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "ema_gratitude",
            "🙏 感激",
            |row| {
                format!(
                    "  {} 对 {} 因「{}」",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "cope_deflect",
            "🔇 应对：回避/转移",
            |row| {
                format!(
                    "  {} 面对「{}」倾向于回避",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "cope_confront",
            "⚡ 应对：对抗/追问",
            |row| {
                format!(
                    "  {} 面对 {} 倾向于对抗",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "cope_endure",
            "🫥 应对：隐忍/接受",
            |row| {
                format!(
                    "  {} 面对「{}」倾向于隐忍",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
        (
            "scene_tension",
            "🔥 场景张力",
            |row| {
                format!(
                    "  {} ↔ {} — {}",
                    row[0],
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(2).map(|s| s.as_str()).unwrap_or("?"),
                )
            },
        ),
    ];

    for (predicate, label, formatter) in predicates {
        if let Some(rows) = reasoning.get(predicate) {
            if !rows.is_empty() {
                has_possibilities = true;
                println!("{}", label.bold());
                for row in rows {
                    println!("{}", formatter(row));
                }
                println!();
            }
        }
    }

    if !has_possibilities {
        println!(
            "{}",
            "(推理引擎未发现叙事可能性 — 检查 cards 是否填充了位置、关系、秘密)"
                .dimmed()
        );
        println!();
    }

    // ── Constraint suggestions for active content node ──────────
    if let Some(content) = content_map.get(&content_id) {
        let mut has_suggestions = false;

        if !content.constraints.exit_state.is_empty() {
            let (ok, failures) = ledger::state::constraint::check_assertions(
                &snapshot,
                &content.constraints.exit_state,
            );
            if !ok {
                has_suggestions = true;
                println!("{}", "═══ 📋 约束建议 ═══".magenta().bold());
                println!();
                for f in &failures {
                    println!("{}: {}", "🎯 exit_state 未满足".red().bold(), f);
                }
                println!();
            }
        }

        if let Some(min) = content.constraints.min_effects {
            let current = content.effects.len() as u32;
            if current < min {
                if !has_suggestions {
                    println!("{}", "═══ 📋 约束建议 ═══".magenta().bold());
                    println!();
                }
                has_suggestions = true;
                println!(
                    "{}: effects {}/{}",
                    "🎇 effects 不足".yellow().bold(),
                    current,
                    min
                );
                println!();
            }
        }

        if let Some((min, max)) = content.constraints.words {
            if content.word_count < min {
                if !has_suggestions {
                    println!("{}", "═══ 📋 约束建议 ═══".magenta().bold());
                    println!();
                }
                has_suggestions = true;
                println!(
                    "{}: {} 字 / 要求 {}-{} 字",
                    "📝 字数不足".yellow().bold(),
                    content.word_count,
                    min,
                    max
                );
                println!();
            }
        }

        if !has_suggestions {
            println!(
                "{}",
                "✨ 目前没有阻断性约束！".green().bold()
            );
        }
    }

    // Record progress
    let mut progress = ledger::Progress::load(&cards_dir, &content_id);
    progress.record_suggest();
    let _ = progress.save(&cards_dir, &content_id);

    Ok(())
}
