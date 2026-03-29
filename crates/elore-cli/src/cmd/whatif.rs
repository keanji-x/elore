use colored::Colorize;
use std::path::Path;

use ledger::effect::diff::SnapshotDiff;
use ledger::effect::op::Op;
use ledger::state::snapshot::Snapshot;

pub async fn run(
    project: &Path,
    chapter: &str,
    effect_dsl: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");

    let op = Op::parse(effect_dsl)?;
    println!("{}", format!("═══ What-If: {chapter} ═══").cyan().bold());
    println!("假设 effect: {}", op.describe());
    println!();

    // Build snapshot WITHOUT the hypothetical effect
    let snap_before = Snapshot::build(chapter, &entities_dir, &everlore_dir)?;

    // Build a simulated snapshot WITH the effect applied
    let mut snap_after = snap_before.clone();
    for entity in &mut snap_after.entities {
        op.apply_to_entity(entity);
    }
    for secret in &mut snap_after.secrets {
        op.apply_to_secret(secret);
    }
    for ge in &mut snap_after.goal_entities {
        op.apply_to_goal(ge);
    }

    let diff = SnapshotDiff::compute(&snap_before, &snap_after);
    if diff.is_empty() {
        println!("{}", "(此 effect 不会产生变化)".dimmed());
    } else {
        println!("{}", diff.render());
        let changed = diff.changed_entity_ids();
        println!(
            "受影响实体: {}",
            changed.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    Ok(())
}
