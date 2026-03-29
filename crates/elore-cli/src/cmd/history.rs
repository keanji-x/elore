use std::path::Path;
use colored::Colorize;

use ledger::effect::history::History;
use ledger::effect::op::Op;

pub fn run(project: &Path, action: crate::HistoryAction) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");

    match action {
        crate::HistoryAction::List { chapter } => {
            let history = History::load(&everlore);
            if history.entries.is_empty() {
                println!("{}", "事件日志为空".dimmed());
                return Ok(());
            }

            let entries: Vec<_> = if let Some(ch) = &chapter {
                history.entries.iter().filter(|e| &e.chapter == ch).collect()
            } else {
                history.entries.iter().collect()
            };

            println!("{}", "═══ History ═══".cyan().bold());
            for entry in &entries {
                println!(
                    "  [{}/{}] {}",
                    entry.chapter.cyan(),
                    entry.seq,
                    entry.effect.describe()
                );
            }
            println!("\n共 {} 条记录", entries.len());
            Ok(())
        }

        crate::HistoryAction::Add { chapter, effect } => {
            commit_effect(project, &chapter, &effect)
        }

        crate::HistoryAction::Rollback { chapter } => {
            let everlore = project.join(".everlore");
            let removed = History::rollback(&everlore, &chapter)?;
            if removed == 0 {
                println!("{}", format!("章节 {chapter} 无事件可回滚").dimmed());
            } else {
                println!("{} 回滚了 {} 的 {} 条事件", "✓".green().bold(), chapter.cyan(), removed);
            }
            Ok(())
        }
    }
}

/// Commit a single effect DSL string. Used by both `history add` and `add effect`.
pub fn commit_effect(
    project: &Path,
    chapter: &str,
    dsl: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let history = History::load(&everlore);
    let seq = history.next_seq(chapter);
    let op = Op::parse(dsl)?;
    let entry = ledger::HistoryEntry {
        chapter: chapter.to_string(),
        seq,
        effect: op.clone(),
    };
    History::append(&everlore, &[entry])?;
    println!("{} [{}/{}] {}", "✓".green().bold(), chapter.cyan(), seq, op.describe());
    Ok(())
}
