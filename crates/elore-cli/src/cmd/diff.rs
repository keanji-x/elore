use std::path::Path;
use colored::Colorize;

use ledger::state::snapshot::Snapshot;
use ledger::effect::diff::SnapshotDiff;

pub async fn run(project: &Path, from: &str, to: &str) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");

    let snap_from = Snapshot::build(from, &entities_dir, &everlore_dir)?;
    let snap_to = Snapshot::build(to, &entities_dir, &everlore_dir)?;

    let diff = SnapshotDiff::compute(&snap_from, &snap_to);
    println!("{}", format!("═══ Diff: {from} → {to} ═══").cyan().bold());
    println!();

    if diff.is_empty() {
        println!("{}", "(无差异)".dimmed());
    } else {
        println!("{}", diff.render());
        let changed = diff.changed_entity_ids();
        println!("影响实体: {}", changed.into_iter().collect::<Vec<_>>().join(", "));
    }

    Ok(())
}
