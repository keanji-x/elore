//! Phase state machine CLI commands:
//! checkout / status / submit / approve / reject

use std::path::Path;

use colored::Colorize;
use serde_json::json;

use ledger::effect::beat::Beat;
use ledger::state::constraint::check_assertions;
use ledger::state::phase::Phase;
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

pub fn status(project: &Path, format: Format) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let state = ProjectState::load(&everlore);

    let phase_id = match state.active_phase() {
        Some(id) => id.to_string(),
        None => {
            // Show plan overview
            return show_plan_overview(project, &state, format);
        }
    };

    let phases_dir = everlore.join("phases");
    let beats_dir = everlore.join("beats");
    let annotations_dir = everlore.join("annotations");
    let entities_dir = everlore.join("entities");

    let phase = Phase::load(&phases_dir, &phase_id)?;
    let beats = Beat::load_phase(&beats_dir, &phase_id);
    let anns = annotation::load_annotations(&annotations_dir, &phase_id);

    // Build snapshot with beats as effects
    let snap = Snapshot::build(&phase_id, &entities_dir, &everlore)?;

    // ── L1: Ledger ──
    let (inv_ok, inv_failures) = check_assertions(&snap, &phase.constraints.ledger.invariants);
    let (exit_ok, exit_failures) = check_assertions(&snap, &phase.constraints.ledger.exit_state);

    // ── L2: Resolver ──
    let total_effects: u32 = beats.iter().map(|b| b.effects.len() as u32).sum();
    let min_effects = phase.constraints.resolver.min_effects.unwrap_or(0);
    let effects_met = total_effects >= min_effects;

    // ── L3: Executor ──
    let total_words = Beat::total_words(&beats);
    let beat_count = beats.len() as u32;
    let plan_count = phase.constraints.executor.writing_plan.len() as u32;
    let (word_min, word_max) = phase.constraints.executor.words.unwrap_or((0, u32::MAX));
    let words_met = total_words >= word_min && total_words <= word_max;

    // ── L4: Evaluator ──
    let avg = annotation::avg_score(&anns);
    let min_avg = phase.constraints.evaluator.min_avg_score.unwrap_or(0.0);
    let low = annotation::low_beats(&anns, 2);
    let max_boring = phase
        .constraints
        .evaluator
        .max_boring_beats
        .unwrap_or(u32::MAX);
    let eval_ok = (anns.is_empty() || avg >= min_avg) && (low.len() as u32) <= max_boring;

    let all_ok = inv_ok && exit_ok && effects_met && words_met && eval_ok;

    match format {
        Format::Json => {
            let j = json!({
                "phase": phase_id,
                "complete": all_ok,
                "beats": beat_count,
                "words": total_words,
                "effects": total_effects,
                "ledger": {
                    "status": if inv_ok && exit_ok { "ok" } else { "partial" },
                    "invariants_passing": inv_ok,
                    "exit_state_met": exit_ok,
                    "exit_state_pending": exit_failures,
                },
                "resolver": {
                    "status": if effects_met { "ok" } else { "partial" },
                    "effects": format!("{total_effects}/{min_effects}"),
                },
                "executor": {
                    "status": if words_met { "ok" } else { "in_progress" },
                    "words": format!("{total_words}/{word_min}-{word_max}"),
                    "beats": format!("{beat_count}/{}", if plan_count > 0 { plan_count } else { beat_count }),
                },
                "evaluator": {
                    "status": if eval_ok { "ok" } else { "needs_revision" },
                    "avg_score": if anns.is_empty() { None } else { Some(avg) },
                    "low_beats": low.iter().map(|(b, s, n)| json!({"beat": b, "score": s, "note": n})).collect::<Vec<_>>(),
                },
            });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        Format::Human => {
            let icon = if all_ok {
                "✓".green()
            } else {
                "◎".yellow()
            };
            println!(
                "{} Phase: {} ({})",
                icon,
                phase_id.cyan().bold(),
                if all_ok {
                    "ALL ✓".green().to_string()
                } else {
                    "进行中".yellow().to_string()
                }
            );
            println!();

            // L1
            let l1 = if inv_ok && exit_ok {
                "✓".green()
            } else {
                "✗".red()
            };
            println!("  {} L1·State", l1);
            if !inv_ok {
                for f in &inv_failures {
                    println!("    {} invariant: {}", "✗".red(), f);
                }
            }
            if !exit_ok {
                for f in &exit_failures {
                    println!("    {} exit_state: {}", "⏳".to_string().dimmed(), f);
                }
            }

            // L2
            let l2 = if effects_met {
                "✓".green()
            } else {
                "◎".yellow()
            };
            println!(
                "  {} L2·Drama  effects: {}/{}",
                l2, total_effects, min_effects
            );

            // L3
            let l3 = if words_met {
                "✓".green()
            } else {
                "◎".yellow()
            };
            println!(
                "  {} L3·Writing words: {}/{}-{}, beats: {}/{}",
                l3,
                total_words,
                word_min,
                word_max,
                beat_count,
                if plan_count > 0 {
                    plan_count.to_string()
                } else {
                    "∞".into()
                }
            );

            // L4
            let l4 = if eval_ok {
                "✓".green()
            } else {
                "◎".yellow()
            };
            if !anns.is_empty() {
                println!(
                    "  {} L4·Reader  avg: {:.1}, low_beats: {}",
                    l4,
                    avg,
                    low.len()
                );
            } else {
                println!("  {} L4·Reader  (尚未标注)", l4);
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
    let mut state = ProjectState::load(&everlore);
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
