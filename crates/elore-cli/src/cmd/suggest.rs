//! `elore suggest`
//!
//! Runs the Datalog reasoning engine on the current snapshot to derive
//! narrative possibilities, then displays L1/L2 constraint suggestions.

use colored::Colorize;
use std::path::Path;

pub async fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let state = ledger::ProjectState::load(&everlore);

    let Some(phase_id) = state.active_phase() else {
        println!("{}", "(没有活跃的 Phase，无法提供建议)".yellow());
        return Ok(());
    };

    let phases_dir = everlore.join("phases");
    let phase = ledger::Phase::load(&phases_dir, phase_id).ok();
    let beats_dir = everlore.join("beats");
    let beats = ledger::Beat::load_phase(&beats_dir, phase_id);
    let snapshot = ledger::Snapshot::build(
        phase_id,
        &everlore.join("entities"),
        &everlore,
    )?;

    // ── Run reasoning engine ────────────────────────────────────
    let reasoning = ledger::Program::from_snapshot(&snapshot, None).run().await?;

    println!("{}", "═══ 🔮 叙事推理引擎 ═══".magenta().bold());
    println!("Phase: {}  |  推导事实: {}", phase_id.cyan(), reasoning.total_facts);
    println!();

    let mut has_possibilities = false;

    // Narrative possibilities from reasoning
    if let Some(rows) = reasoning.get("can_meet") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "📍 可能的相遇".blue().bold());
            for row in rows {
                println!("  {} ↔ {}", row[0].green(), row[1].green());
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("active_danger") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "⚔ 危险遭遇（有主动敌意的角色相遇）".red().bold());
            for row in rows {
                println!("  {} → {}", row[0].red(), row[1].red());
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("armed_danger") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "🔪 定向威胁（有明确攻击意图 + 能见面）".red().bold());
            for row in rows {
                println!("  {} → {}", row[0].red(), row[1].red());
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("protector") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "🛡 守护者（愿意牺牲 + 保护对象正受威胁）".green().bold());
            for row in rows {
                println!("  {} 守护 {}", row[0].green(), row[1].green());
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("betrayal_opportunity") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "🗡 背叛时机（持有秘密 + 能见面）".red().bold());
            for row in rows {
                println!(
                    "  {} 可对 {} 发动（秘密: {}）",
                    row[0].yellow(),
                    row[1].yellow(),
                    row[2].dimmed()
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("critical_reveal") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "💬 关键揭秘（高优先级信息传递）".cyan().bold());
            for row in rows {
                println!(
                    "  {} 可向 {} 透露「{}」",
                    row[1].green(),
                    row[2].green(),
                    row[0].dimmed()
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("info_cascade") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "🔗 信息级联（秘密传播路径）".blue().bold());
            for row in rows {
                println!(
                    "  「{}」: {} → {}",
                    row[0].dimmed(),
                    row[1].green(),
                    row[2].green()
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("dramatic_irony") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!(
                "{}",
                "🎭 戏剧性反讽（读者知道但角色不知道）".magenta().bold()
            );
            for row in rows {
                println!(
                    "  {} 对「{}」一无所知",
                    row[1].yellow(),
                    row[0].dimmed()
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("alliance_opportunity") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "🤝 联盟机会（共同欲望）".green().bold());
            for row in rows {
                println!(
                    "  {} & {} — 共同渴望: {}",
                    row[0].green(),
                    row[1].green(),
                    row[2].dimmed()
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("suspense") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!("{}", "❓ 悬念（活跃目标无解法）".yellow().bold());
            for row in rows {
                println!("  {}/{}", row[0].yellow(), row[1]);
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("goal_conflict_encounter") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!(
                "{}",
                "💥 目标冲突相遇（冲突双方能见面）".red().bold()
            );
            for row in rows {
                println!(
                    "  {}/{} ↔ {}/{}",
                    row[0].red(),
                    row[1],
                    row[2].red(),
                    row[3]
                );
            }
            println!();
        }
    }

    if let Some(rows) = reasoning.get("orphaned_secret") {
        if !rows.is_empty() {
            has_possibilities = true;
            println!(
                "{}",
                "⚠ 孤立秘密（读者知道但无角色知晓，无法在故事中揭示）"
                    .yellow()
                    .bold()
            );
            for row in rows {
                println!(
                    "  「{}」→ 需要安排角色发现此秘密，否则永远无法推动剧情",
                    row[0].yellow()
                );
            }
            println!();
        }
    }

    if !has_possibilities {
        println!(
            "{}",
            "(推理引擎未发现叙事可能性 — 检查 cards 是否填充了位置、关系、秘密)".dimmed()
        );
        println!();
    }

    // ── Existing L1/L2 constraint suggestions ───────────────────

    if let Some(phase) = &phase {
        let dummy_eval = ledger::EvaluatorInput {
            annotations_count: 0,
            avg_score: None,
            low_beats: vec![],
            present_tags: vec![],
        };
        let progress = phase.evaluate_progress(&snapshot, &beats, &dummy_eval);

        println!("{}", "═══ 📋 约束建议 ═══".magenta().bold());
        println!();

        let mut has_suggestions = false;

        // L1 Exit States
        if !progress.ledger.exit_state_met {
            for pending in &progress.ledger.exit_state_pending {
                has_suggestions = true;
                let (query, expected) = parse_assertion_failure(pending);

                if query.contains(".has_item(") {
                    let parts: Vec<&str> = query
                        .split(&['.', '(', ')'][..])
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.len() >= 3 {
                        let entity = parts[0];
                        let item = parts[2];
                        println!(
                            "{}: 剧情缺少关键道具的获取。",
                            "🎯 剧作目标".red().bold()
                        );
                        println!(
                        "  -> {entity} 必须从某处获得物品 `{item}`！你可以安排一场战斗或寻宝。"
                    );
                        if expected == Some("true") {
                            println!(
                                "  -> 在 Markdown 顶部插入 YAML 动作: {}({}, {})",
                                "add_item".green(),
                                entity,
                                item
                            );
                        } else {
                            println!(
                                "  -> 在 Markdown 顶部插入 YAML 动作: {}({}, {})",
                                "remove_item".green(),
                                entity,
                                item
                            );
                        }
                    } else {
                        println!("{}: 满足状态 {}", "🎯 剧作目标".red().bold(), query);
                    }
                } else if query.starts_with("knows(") {
                    let parts: Vec<&str> = query
                        .split(&['(', ')', ',', ' '][..])
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.len() >= 3 {
                        let entity = parts[1];
                        let secret = parts[2];
                        println!(
                            "{}: 剧情缺少关键信息的揭露。",
                            "🎯 剧作目标".red().bold()
                        );
                        println!(
                        "  -> {entity} 必须得知秘密 `{secret}`！你可以设计一场对话或者发现一本日记。"
                    );
                        println!(
                            "  -> 在 Markdown 顶部插入 YAML 动作: {}({}, {})",
                            "reveal".green(),
                            secret,
                            entity
                        );
                    }
                } else if query.starts_with("entity_alive(") {
                    println!("{}: 角色的生死等关键性变化。", "🎯 剧作目标".red().bold());
                    println!(
                        "  -> 修改状态: {} = {}",
                        query,
                        expected.unwrap_or("true")
                    );
                } else {
                    println!(
                        "{}: 剧情必须达成以下状态变动: {} (期望值: {})",
                        "🎯 剧作目标".red().bold(),
                        query,
                        expected.unwrap_or("true")
                    );
                }
                println!();
            }
        }

        // L2 Min Effects
        let c_resolver = &phase.constraints.resolver;
        let effects_so_far = ledger::Beat::all_effects(&beats).len();
        if let Some(min) = c_resolver.min_effects {
            if effects_so_far < min as usize {
                has_suggestions = true;
                println!("{}: 剧烈变动不足。", "🎇 冲突构建".yellow().bold());
                println!(
                    "  -> 当前 YAML Ops 数量: {} / 要求: {}",
                    effects_so_far, min
                );
                println!("  -> 建议: 引入更多的关系破裂、特质增加、物品得失。例如使用 add_rel(), remove_item(), 或 add_trait() 以制造戏剧张力。");
                println!();
            }
        }

        if !has_suggestions {
            println!(
                "{}",
                "✨ 目前剧情进展顺利，没有 L1/L2 的阻断性约束！"
                    .green()
                    .bold()
            );
        }
    }

    Ok(())
}

fn parse_assertion_failure(f: &str) -> (&str, Option<&str>) {
    if let Some(idx) = f.find(" ≠ ") {
        (&f[0..idx], Some(&f[idx + 5..]))
    } else {
        (f, None)
    }
}
