use std::path::Path;
use colored::Colorize;

use ledger::state::snapshot::Snapshot;
use resolver::drama;
use resolver::validate as val;

pub async fn run(project: &Path, chapter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");
    let snap = Snapshot::build(chapter, &entities_dir, &everlore_dir)?;
    let drama_node = drama::load_drama(&everlore_dir, chapter)?;
    let verdict = val::validate(&snap, &drama_node, None);

    println!("{}", format!("═══ Validate: {chapter} ═══").cyan().bold());
    println!();
    println!("{}", verdict.render());

    if !drama_node.dramatic_intents.is_empty() {
        println!("\nDrama intents:");
        for (i, intent) in drama_node.dramatic_intents.iter().enumerate() {
            let status = if verdict.is_accept() { "✓".green() } else { "?".yellow() };
            println!("  {status} {}. {}", i + 1, intent.summary());
        }
    } else {
        println!("{}", "  (无 drama node — 使用默认空配置)".dimmed());
        println!("  → 创建 .everlore/drama/{}.yaml 来定义戏剧意图", chapter);
    }

    Ok(())
}
