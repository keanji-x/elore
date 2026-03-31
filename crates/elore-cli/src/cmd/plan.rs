use colored::Colorize;
use std::path::Path;

use ledger::input::goal;
use ledger::state::phase_manager::ProjectState;
use ledger::state::snapshot::Snapshot;

pub async fn run(project: &Path, phase: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");
    let state = ProjectState::load(&everlore_dir);

    let phase_id = if let Some(phase_id) = phase {
        println!("{}", format!("使用 phase 视角: {phase_id}").cyan().bold());
        phase_id.to_string()
    } else if let Some(active_phase) = state.active_phase() {
        println!(
            "{}",
            format!("使用当前 active phase: {active_phase}")
                .cyan()
                .bold()
        );
        active_phase.to_string()
    } else {
        return Err("没有活跃的 phase，可用 --phase <id> 指定".into());
    };
    let snap = Snapshot::build(&phase_id, &entities_dir, &everlore_dir)?;

    println!("{}", "═══ 态势面板 ═══".cyan().bold());

    // Suspense
    let suspense = goal::find_suspense(&snap.goal_entities);
    if !suspense.is_empty() {
        println!("\n{}", "悬念 (未解目标):".yellow().bold());
        for fg in &suspense {
            let problem = fg.goal.problem.as_deref().unwrap_or("—");
            println!(
                "  ● {}/{}: {} — {}",
                fg.owner, fg.goal.id, fg.goal.want, problem
            );
        }
    }

    // Active conflicts
    let conflicts = goal::find_active_conflicts(&snap.goal_entities);
    if !conflicts.is_empty() {
        println!("\n{}", "活跃冲突:".red().bold());
        for (a, b) in &conflicts {
            println!(
                "  ⚔ {}/{} ({}) ↔ {}/{} ({})",
                a.owner, a.goal.id, a.goal.want, b.owner, b.goal.id, b.goal.want
            );
        }
    }

    // Secrets
    if !snap.secrets.is_empty() {
        println!("\n{}", "信息不对称:".magenta().bold());
        for s in &snap.secrets {
            let tech = s.classify();
            let known = if s.known_by.is_empty() {
                "无人".to_string()
            } else {
                s.known_by.join(", ")
            };
            let reader = if s.revealed_to_reader { "✓" } else { "✗" };
            println!(
                "  {} — {:?} [已知: {} | 读者: {}]",
                s.id, tech, known, reader
            );
        }
    }

    if suspense.is_empty() && conflicts.is_empty() && snap.secrets.is_empty() {
        println!("\n{}", "(无悬念、冲突或秘密)".dimmed());
    }

    // Reasoning engine
    let reasoning = ledger::Program::from_snapshot(&snap, None).run().await?;
    if reasoning.total_facts > 0 {
        println!("\n{}", format!("推理引擎 ({} 推导事实):", reasoning.total_facts).blue().bold());
        if let Some(rows) = reasoning.get("can_meet") {
            if !rows.is_empty() {
                let pairs: Vec<String> = rows
                    .iter()
                    .filter(|r| r[0] < r[1])
                    .map(|r| format!("{}↔{}", r[0], r[1]))
                    .collect();
                if !pairs.is_empty() {
                    println!("  📍 相遇: {}", pairs.join(", "));
                }
            }
        }
        if let Some(rows) = reasoning.get("betrayal_opportunity") {
            if !rows.is_empty() {
                for row in rows {
                    println!("  🗡 背叛: {} → {} ({})", row[0], row[1], row[2]);
                }
            }
        }
        if let Some(rows) = reasoning.get("dramatic_irony") {
            let count = rows.len();
            if count > 0 {
                println!("  🎭 戏剧反讽: {} 条", count);
            }
        }
        if let Some(rows) = reasoning.get("alliance_opportunity") {
            if !rows.is_empty() {
                for row in rows {
                    println!("  🤝 联盟: {} & {} ({})", row[0], row[1], row[2]);
                }
            }
        }
    }

    Ok(())
}
