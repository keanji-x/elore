//! Phase state machine CLI commands:
//! checkout / status / submit / approve / reject

use std::path::Path;

use colored::Colorize;
use serde_json::json;

use ledger::effect::beat::Beat;
use ledger::state::phase::{DefinitionStatus, LayerStatus, LowBeat, Phase, PhaseProgress};
use ledger::state::phase_manager::ProjectState;
use ledger::state::snapshot::Snapshot;

use evaluator::annotation;

use crate::cmd::read_query::Format;

// ══════════════════════════════════════════════════════════════════
// checkout
// ══════════════════════════════════════════════════════════════════

pub fn checkout(project: &Path, phase_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases_dir = everlore.join("phases");

    // Verify phase definition exists
    let _phase = Phase::load(&phases_dir, phase_id)?;

    let mut state = ProjectState::load(&everlore);
    state.checkout(phase_id)?;
    state.save(&everlore)?;

    println!(
        "{} checkout → {}",
        "✓".green().bold(),
        phase_id.cyan().bold()
    );
    println!("  使用 {} 查看当前约束进度", "elore status".bold());
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// status
// ══════════════════════════════════════════════════════════════════

pub fn status(
    project: &Path,
    phase_filter: Option<&str>,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let state = ProjectState::load(&everlore);

    let phase_id = phase_filter
        .map(str::to_string)
        .or_else(|| state.active_phase().map(str::to_string));
    let phase_id = match phase_id {
        Some(id) => id,
        None => {
            // Show plan overview
            return show_plan_overview(project, &state, format);
        }
    };

    let phases_dir = everlore.join("phases");
    let beats_dir = everlore.join("beats");
    let annotations_dir = everlore.join("annotations");
    let entities_dir = everlore.join("entities");

    let progress = build_phase_progress(
        &everlore,
        &phases_dir,
        &beats_dir,
        &annotations_dir,
        &entities_dir,
        &phase_id,
    )?;

    match format {
        Format::Json => {
            let j = json!({
                "phase": progress.phase_id,
                "complete": progress.complete,
                "definition_status": progress.definition_status,
                "beats": progress.beats,
                "words": progress.words,
                "effects": progress.effects_count,
                "blockers": progress.blockers,
                "ledger": progress.ledger,
                "resolver": progress.resolver,
                "executor": progress.executor,
                "evaluator": progress.evaluator,
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            let icon = if progress.complete {
                "✓".green()
            } else {
                "◎".yellow()
            };
            println!(
                "{} Phase: {} ({})",
                icon,
                progress.phase_id.cyan().bold(),
                if progress.complete {
                    "ALL ✓".green().to_string()
                } else {
                    "进行中".yellow().to_string()
                }
            );
            println!(
                "  definition: {}",
                render_definition_status(&progress.definition_status)
            );
            println!();

            println!(
                "  {} L1·State  exit_state_pending: {}",
                render_layer_status(&progress.ledger.status),
                progress.ledger.exit_state_pending.len()
            );
            println!(
                "  {} L2·Drama  effects: {}",
                render_layer_status(&progress.resolver.status),
                progress.resolver.effects
            );
            if !progress.resolver.intents_pending.is_empty() {
                println!(
                    "    intents_pending: {}",
                    progress.resolver.intents_pending.len()
                );
            }
            println!(
                "  {} L3·Writing words: {}, beats: {}",
                render_layer_status(&progress.executor.status),
                progress.executor.words,
                progress.executor.beats
            );
            if !progress.executor.beats_remaining.is_empty() {
                println!(
                    "    beats_remaining: {}",
                    progress.executor.beats_remaining.len()
                );
            }
            println!(
                "  {} L4·Reader  avg: {}, low_beats: {}",
                render_layer_status(&progress.evaluator.status),
                progress
                    .evaluator
                    .avg_score
                    .map(|score| format!("{score:.1}"))
                    .unwrap_or_else(|| "—".into()),
                progress.evaluator.low_beats.len()
            );
            if !progress.evaluator.required_tags_missing.is_empty() {
                println!(
                    "    required_tags_missing: {:?}",
                    progress.evaluator.required_tags_missing
                );
            }
            if !progress.blockers.is_empty() {
                println!();
                println!("{}", "  blockers:".yellow().bold());
                for blocker in &progress.blockers {
                    println!("    - {blocker}");
                }
            }
        }
    }

    Ok(())
}

fn show_plan_overview(
    _project: &Path,
    state: &ProjectState,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        Format::Json => {
            let j = json!({
                "current_phase": state.current_phase,
                "plan": state.plan,
                "phases": state.phases,
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            if state.plan.is_empty() {
                println!(
                    "{}",
                    "(尚无 phase 计划，请先 `elore add phase '{...}'`)".dimmed()
                );
                return Ok(());
            }
            println!("{}", "═══ Phase Plan ═══".cyan().bold());
            for id in &state.plan {
                if let Some(entry) = state.phases.get(id) {
                    let icon = match entry.status {
                        ledger::PhaseStatus::Locked => "🔒",
                        ledger::PhaseStatus::Ready => "⬜",
                        ledger::PhaseStatus::Active => "🔵",
                        ledger::PhaseStatus::Reviewing => "🟡",
                        ledger::PhaseStatus::Approved => "✅",
                    };
                    println!(
                        "  {} {} ({:?}) — {} words, {} beats",
                        icon,
                        id.bold(),
                        entry.status,
                        entry.words,
                        entry.beats
                    );
                }
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// submit / approve / reject
// ══════════════════════════════════════════════════════════════════

pub fn submit(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases_dir = everlore.join("phases");
    let beats_dir = everlore.join("beats");
    let annotations_dir = everlore.join("annotations");
    let entities_dir = everlore.join("entities");
    let mut state = ProjectState::load(&everlore);
    let phase_id = state
        .active_phase()
        .ok_or_else(|| "没有活跃的 phase".to_string())?
        .to_string();
    let progress = build_phase_progress(
        &everlore,
        &phases_dir,
        &beats_dir,
        &annotations_dir,
        &entities_dir,
        &phase_id,
    )?;
    if !progress.complete {
        println!(
            "{} {} 不能 submit",
            "✗".red().bold(),
            phase_id.cyan().bold()
        );
        println!(
            "  definition: {}",
            render_definition_status(&progress.definition_status)
        );
        for blocker in &progress.blockers {
            println!("  - {blocker}");
        }
        return Err("phase 约束尚未满足，submit 已拒绝".into());
    }
    let phase_id = state.submit()?;
    state.save(&everlore)?;
    println!(
        "{} {} → reviewing",
        "✓".green().bold(),
        phase_id.cyan().bold()
    );
    println!(
        "  使用 {} 或 {} 来审阅",
        "elore approve".bold(),
        "elore reject \"原因\"".bold()
    );
    Ok(())
}

pub fn approve(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases_dir = everlore.join("phases");
    let mut state = ProjectState::load(&everlore);
    let phase_id = state.approve()?;

    // Resolve deps
    let all_phases: Vec<Phase> = Phase::list(&phases_dir)
        .into_iter()
        .filter_map(|id| Phase::load(&phases_dir, &id).ok())
        .collect();
    state.resolve_dependencies_with_phases(&all_phases);

    state.save(&everlore)?;
    println!(
        "{} {} → approved ✅",
        "✓".green().bold(),
        phase_id.cyan().bold()
    );

    // Check if any phase was unlocked
    for (id, entry) in &state.phases {
        if entry.status == ledger::PhaseStatus::Ready && id != &phase_id {
            println!("  {} {} 已解锁", "→".dimmed(), id.bold());
        }
    }
    Ok(())
}

pub fn reject(project: &Path, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let mut state = ProjectState::load(&everlore);
    let phase_id = state.reject(reason)?;
    state.save(&everlore)?;
    println!(
        "{} {} → active (rejected)",
        "↩".yellow().bold(),
        phase_id.cyan().bold()
    );
    println!("  原因: {}", reason);
    Ok(())
}

fn build_phase_progress(
    everlore: &Path,
    phases_dir: &Path,
    beats_dir: &Path,
    annotations_dir: &Path,
    entities_dir: &Path,
    phase_id: &str,
) -> Result<PhaseProgress, Box<dyn std::error::Error>> {
    let phase = Phase::load(phases_dir, phase_id)?;
    let beats = Beat::load_phase(beats_dir, phase_id);
    let anns = annotation::load_annotations(annotations_dir, phase_id);
    let snap = Snapshot::build(phase_id, entities_dir, everlore)?;
    let avg_score = if anns.is_empty() {
        None
    } else {
        Some(annotation::avg_score(&anns))
    };
    let low_beats = annotation::low_beats(&anns, 2)
        .into_iter()
        .map(|(beat, score, note)| LowBeat {
            beat,
            score,
            reason: note,
        })
        .collect();
    let present_tags = anns
        .iter()
        .flat_map(|ann| ann.tags.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(phase.evaluate_progress(
        &snap,
        &beats,
        &ledger::EvaluatorInput {
            annotations_count: anns.len() as u32,
            avg_score,
            low_beats,
            present_tags,
        },
    ))
}

fn render_layer_status(status: &LayerStatus) -> colored::ColoredString {
    match status {
        LayerStatus::Ok => "✓".green(),
        LayerStatus::Partial => "◎".yellow(),
        LayerStatus::InProgress => "◎".yellow(),
        LayerStatus::NeedsRevision => "✗".red(),
        LayerStatus::NotChecked => "…".dimmed(),
    }
}

fn render_definition_status(status: &DefinitionStatus) -> colored::ColoredString {
    match status {
        DefinitionStatus::Explicit => "explicit".green(),
        DefinitionStatus::Derived => "derived".green(),
        DefinitionStatus::Partial => "partial".yellow(),
        DefinitionStatus::Missing => "missing".red(),
    }
}
