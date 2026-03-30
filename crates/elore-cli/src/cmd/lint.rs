//! `elore lint-drafts`
//!
//! Statically validates all markdown draft frontmatters (YAML) for:
//! 1. Valid JSON/YAML structure
//! 2. Syntactically valid Op definitions
//! 3. Semantically valid entity/secret IDs

use colored::Colorize;
use ledger::effect::op::Op;
use ledger::LedgerError;
use std::collections::HashSet;
use std::path::Path;

pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "═══ Linting Drafts ═══".cyan().bold());

    // 1. Load valid entity IDs and secret IDs
    let everlore_dir = project.join(".everlore");
    let entities_dir = everlore_dir.join("entities");
    let mut valid_entities = HashSet::new();
    let mut valid_secrets = HashSet::new();

    if entities_dir.exists() {
        let entities = ledger::input::entity::load_entities(&entities_dir)?;
        for e in entities {
            valid_entities.insert(e.id().to_string());
        }
    }
    if everlore_dir.exists() {
        let secrets = ledger::input::secret::load_secrets(&everlore_dir)?;
        for s in secrets {
            valid_secrets.insert(s.id);
        }
    }

    // Include implicit/global entities like 'reader' if needed
    // "reader" is mostly assumed, or we don't care about it, but well...

    // 2. Scan all drafts
    let drafts_dir = project.join("drafts");
    if !drafts_dir.exists() {
        println!("{}", "No drafts directory found.".yellow());
        return Ok(());
    }

    let mut drafts = Vec::new();
    find_markdown_files(&drafts_dir, &mut drafts);

    let mut has_errors = false;
    let mut total_drafts = 0;
    let mut total_ops = 0;

    for draft in drafts {
        total_drafts += 1;
        let content = match std::fs::read_to_string(&draft) {
            Ok(c) => c,
            Err(e) => {
                println!("{}: Failed to read file: {}", format_path(&draft, project).red(), e);
                has_errors = true;
                continue;
            }
        };

        let (fm_val, _) = parse_frontmatter(&content);
        if fm_val.is_null() {
            continue; // no frontmatter, skip
        }

        // Check effects
        if let Some(effects_arr) = fm_val.get("effects").and_then(|v| v.as_array()) {
            for (idx, effect_val) in effects_arr.iter().enumerate() {
                if let Some(effect_str) = effect_val.as_str() {
                    total_ops += 1;
                    match Op::parse(effect_str) {
                        Ok(op) => {
                            // Semantic checks
                            let t_entities = op.extract_entities();
                            for e_ref in t_entities {
                                if !valid_entities.contains(e_ref) && e_ref != "reader" && e_ref != "world" {
                                    println!(
                                        "{}: {} in effects[{}] -> {}",
                                        format_path(&draft, project).red(),
                                        "Unknown Entity".red().bold(),
                                        idx,
                                        e_ref.yellow()
                                    );
                                    has_errors = true;
                                }
                            }

                            let t_secrets = op.extract_secrets();
                            for s_ref in t_secrets {
                                if !valid_secrets.contains(s_ref) {
                                    println!(
                                        "{}: {} in effects[{}] -> {}",
                                        format_path(&draft, project).red(),
                                        "Unknown Secret".red().bold(),
                                        idx,
                                        s_ref.yellow()
                                    );
                                    has_errors = true;
                                }
                            }
                        }
                        Err(LedgerError::EffectParse(e)) => {
                            println!(
                                "{}: {} in effects[{}]:\n  -> '{}'\n  Error: {}",
                                format_path(&draft, project).red(),
                                "Syntax Error".red().bold(),
                                idx,
                                effect_str,
                                e.yellow()
                            );
                            has_errors = true;
                        }
                        Err(e) => {
                            println!("{}: Error: {}", format_path(&draft, project).red(), e);
                            has_errors = true;
                        }
                    }
                }
            }
        }
    }

    println!();
    if has_errors {
        println!("{}", format!("Lint Failed: Please fix the errors above before running `elore ingest`. Scanned {} drafts, {} ops.", total_drafts, total_ops).red().bold());
        std::process::exit(1);
    } else {
        println!("{}", format!("Lint Passed: Scanned {} drafts containing {} valid Ops. No syntax or semantic errors found.", total_drafts, total_ops).green());
    }

    Ok(())
}

fn find_markdown_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_markdown_files(&path, files);
            } else if path.extension().is_some_and(|e| e == "md") {
                files.push(path);
            }
        }
    }
}

fn format_path(path: &Path, project: &Path) -> String {
    path.strip_prefix(project)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn parse_frontmatter(raw: &str) -> (serde_json::Value, String) {
    let raw = raw.trim_start();
    if !raw.starts_with("---\n") && !raw.starts_with("---\r\n") {
        return (serde_json::Value::Null, raw.to_string());
    }

    let end_marker = "\n---\n";
    let end_marker_rn = "\r\n---\r\n";
    let (end_idx, marker_len) = if let Some(idx) = raw[4..].find(end_marker) {
        (idx, end_marker.len())
    } else if let Some(idx) = raw[4..].find(end_marker_rn) {
        (idx, end_marker_rn.len())
    } else {
        return (serde_json::Value::Null, raw.to_string());
    };

    let fm_str = &raw[4..4 + end_idx];
    let body_str = &raw[4 + end_idx + marker_len..];

    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(fm_str) {
        // convert yaml to json value for simplicity
        let jstr = serde_json::to_string(&val).unwrap_or_else(|_| "null".to_string());
        let jval = serde_json::from_str(&jstr).unwrap_or(serde_json::Value::Null);
        (jval, body_str.to_string())
    } else {
        (serde_json::Value::Null, body_str.to_string())
    }
}
