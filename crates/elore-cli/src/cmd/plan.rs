use std::path::Path;
use colored::Colorize;

use ledger::input::goal;
use ledger::state::snapshot::Snapshot;

pub async fn run(project: &Path, chapter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");

    let ch = chapter.unwrap_or("latest");
    let snap = Snapshot::build(ch, &entities_dir, &everlore_dir)?;

    println!("{}", "═══ 态势面板 ═══".cyan().bold());

    // Suspense
    let suspense = goal::find_suspense(&snap.goal_entities);
    if !suspense.is_empty() {
        println!("\n{}", "悬念 (未解目标):".yellow().bold());
        for fg in &suspense {
            let problem = fg.goal.problem.as_deref().unwrap_or("—");
            println!("  ● {}/{}: {} — {}", fg.owner, fg.goal.id, fg.goal.want, problem);
        }
    }

    // Active conflicts
    let conflicts = goal::find_active_conflicts(&snap.goal_entities);
    if !conflicts.is_empty() {
        println!("\n{}", "活跃冲突:".red().bold());
        for (a, b) in &conflicts {
            println!("  ⚔ {}/{} ({}) ↔ {}/{} ({})",
                a.owner, a.goal.id, a.goal.want,
                b.owner, b.goal.id, b.goal.want);
        }
    }

    // Secrets
    if !snap.secrets.is_empty() {
        println!("\n{}", "信息不对称:".magenta().bold());
        for s in &snap.secrets {
            let tech = s.classify();
            let known = if s.known_by.is_empty() { "无人".to_string() } else { s.known_by.join(", ") };
            let reader = if s.revealed_to_reader { "✓" } else { "✗" };
            println!("  {} — {:?} [已知: {} | 读者: {}]", s.id, tech, known, reader);
        }
    }

    if suspense.is_empty() && conflicts.is_empty() && snap.secrets.is_empty() {
        println!("\n{}", "(无悬念、冲突或秘密)".dimmed());
    }

    Ok(())
}
