//! `elore pack` — expansion pack management.
//!
//! A pack is a directory containing:
//!   pack.yaml   — metadata manifest
//!   cards/      — pre-built entity/secret/beat cards
//!
//! Install copies cards into the project's cards/ directory.

use std::path::{Path, PathBuf};

use colored::Colorize;
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════
// Pack manifest
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct PackManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub era: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl PackManifest {
    pub fn load(pack_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let manifest_path = pack_dir.join("pack.yaml");
        if !manifest_path.exists() {
            return Err(format!("pack.yaml 不存在: {}", manifest_path.display()).into());
        }
        let raw = std::fs::read_to_string(&manifest_path)?;
        let manifest: PackManifest = serde_yaml::from_str(&raw)?;
        Ok(manifest)
    }
}

// ══════════════════════════════════════════════════════════════════
// Pack discovery
// ══════════════════════════════════════════════════════════════════

/// Search paths for packs: project/packs/ then ~/.elore/packs/
fn discover_pack_dirs(project: &Path) -> Vec<PathBuf> {
    let mut roots = vec![project.join("packs")];

    if let Some(home) = dirs_home() {
        roots.push(home.join(".elore").join("packs"));
    }

    roots
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Find all packs across search paths.
fn find_all_packs(project: &Path) -> Vec<(PackManifest, PathBuf)> {
    let mut packs = Vec::new();
    for root in discover_pack_dirs(project) {
        if !root.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("pack.yaml").exists() {
                    if let Ok(manifest) = PackManifest::load(&path) {
                        packs.push((manifest, path));
                    }
                }
            }
        }
    }
    packs.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    packs
}

/// Resolve a pack name or path to a pack directory.
fn resolve_pack(project: &Path, name_or_path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let as_path = PathBuf::from(name_or_path);

    // Direct path
    if as_path.is_dir() && as_path.join("pack.yaml").exists() {
        return Ok(as_path);
    }

    // Search by id
    for root in discover_pack_dirs(project) {
        let candidate = root.join(name_or_path);
        if candidate.is_dir() && candidate.join("pack.yaml").exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "找不到扩展包 '{name_or_path}' — 检查 packs/ 目录或使用完整路径"
    )
    .into())
}

// ══════════════════════════════════════════════════════════════════
// Commands
// ══════════════════════════════════════════════════════════════════

/// `elore pack list`
pub fn list(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let packs = find_all_packs(project);

    if packs.is_empty() {
        println!("没有找到扩展包。");
        println!("  扩展包搜索路径:");
        for root in discover_pack_dirs(project) {
            println!("    {}", root.display());
        }
        return Ok(());
    }

    println!("{}", "═══ 可用扩展包 ═══".cyan().bold());
    for (manifest, path) in &packs {
        println!();
        println!(
            "  {} {}",
            manifest.id.bold(),
            format!("({})", manifest.name).dimmed()
        );
        if !manifest.description.is_empty() {
            println!("    {}", manifest.description);
        }
        if !manifest.era.is_empty() {
            println!("    时代: {}", manifest.era);
        }
        if !manifest.tags.is_empty() {
            println!("    标签: {}", manifest.tags.join(", "));
        }
        println!("    路径: {}", path.display());
    }

    Ok(())
}

/// `elore pack info <name>`
pub fn info(project: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pack_dir = resolve_pack(project, name)?;
    let manifest = PackManifest::load(&pack_dir)?;
    let cards_dir = pack_dir.join("cards");

    println!("{}", "═══ 扩展包详情 ═══".cyan().bold());
    println!("  ID:   {}", manifest.id.bold());
    println!("  名称: {}", manifest.name);
    if !manifest.description.is_empty() {
        println!("  简介: {}", manifest.description);
    }
    if !manifest.era.is_empty() {
        println!("  时代: {}", manifest.era);
    }
    if !manifest.tags.is_empty() {
        println!("  标签: {}", manifest.tags.join(", "));
    }

    // Count cards
    if cards_dir.exists() {
        println!();
        println!("  {}", "内容:".underline());
        for (subdir, label) in &[
            ("characters", "角色"),
            ("locations", "地点"),
            ("factions", "势力"),
            ("secrets", "秘密"),
        ] {
            let dir = cards_dir.join(subdir);
            if dir.is_dir() {
                let count = count_md_files(&dir);
                if count > 0 {
                    // List file names
                    let names = list_md_ids(&dir);
                    println!("    {} {label}: {count}  {}", "•".green(), names.join(", ").dimmed());
                }
            }
        }

        // Phases
        let phases_dir = cards_dir.join("phases");
        if phases_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&phases_dir) {
                let mut phase_names: Vec<String> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect();
                phase_names.sort();
                if !phase_names.is_empty() {
                    println!(
                        "    {} 阶段: {}  {}",
                        "•".green(),
                        phase_names.len(),
                        phase_names.join(", ").dimmed()
                    );
                }
            }
        }
    }

    Ok(())
}

/// `elore pack install <name>`
pub fn install(project: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pack_dir = resolve_pack(project, name)?;
    let manifest = PackManifest::load(&pack_dir)?;
    let pack_cards = pack_dir.join("cards");

    if !pack_cards.exists() {
        return Err(format!("扩展包 '{}' 没有 cards/ 目录", manifest.id).into());
    }

    let project_cards = project.join("cards");
    if !project_cards.exists() {
        return Err("cards/ 目录不存在 — 请先运行 `elore init`".into());
    }

    println!(
        "{} 安装扩展包: {} ({})",
        "▸".cyan(),
        manifest.id.bold(),
        manifest.name
    );

    // Check for ID conflicts before copying anything
    let conflicts = check_conflicts(&pack_cards, &project_cards)?;
    if !conflicts.is_empty() {
        eprintln!("{} 发现 ID 冲突:", "✗".red().bold());
        for c in &conflicts {
            eprintln!("  {} {c}", "•".red());
        }
        return Err(format!("{} 个实体 ID 冲突，安装已取消", conflicts.len()).into());
    }

    // Copy cards
    let mut copied = 0u32;
    for subdir in &[
        "characters",
        "locations",
        "factions",
        "secrets",
    ] {
        let src = pack_cards.join(subdir);
        if !src.is_dir() {
            continue;
        }
        let dst = project_cards.join(subdir);
        std::fs::create_dir_all(&dst)?;
        copied += copy_md_files(&src, &dst)?;
    }

    // Copy phase directories
    let src_phases = pack_cards.join("phases");
    if src_phases.is_dir() {
        let dst_phases = project_cards.join("phases");
        std::fs::create_dir_all(&dst_phases)?;
        if let Ok(entries) = std::fs::read_dir(&src_phases) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let phase_name = entry.file_name();
                    let dst_phase = dst_phases.join(&phase_name);
                    if dst_phase.exists() {
                        eprintln!(
                            "  {} 阶段 {} 已存在，跳过",
                            "⚠".yellow(),
                            phase_name.to_string_lossy()
                        );
                        continue;
                    }
                    std::fs::create_dir_all(&dst_phase)?;
                    copied += copy_md_files(&path, &dst_phase)?;
                }
            }
        }
    }

    // Copy phase definitions (.yaml) if present
    let pack_everlore_phases = pack_dir.join("phases");
    if pack_everlore_phases.is_dir() {
        let dst_phases = project.join(".everlore").join("phases");
        std::fs::create_dir_all(&dst_phases)?;
        if let Ok(entries) = std::fs::read_dir(&pack_everlore_phases) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "yaml") {
                    let dst = dst_phases.join(entry.file_name());
                    if !dst.exists() {
                        std::fs::copy(&path, &dst)?;
                        println!(
                            "  {} phase 定义: {}",
                            "✓".green(),
                            entry.file_name().to_string_lossy()
                        );
                    }
                }
            }
        }
    }

    println!();
    println!(
        "{} 安装完成: {} 个文件已复制",
        "✓".green().bold(),
        copied
    );
    println!("  运行 {} 编译新卡片", "elore build".cyan());

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

fn count_md_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path().extension().is_some_and(|ext| ext == "md")
                        && !e.file_name().to_string_lossy().starts_with('_')
                })
                .count()
        })
        .unwrap_or(0)
}

fn list_md_ids(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "md")
                && !e.file_name().to_string_lossy().starts_with('_')
        })
        .filter_map(|e| {
            e.path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
        })
        .collect();
    ids.sort();
    ids
}

/// Check for entity ID conflicts between pack and project.
fn check_conflicts(
    pack_cards: &Path,
    project_cards: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut conflicts = Vec::new();

    for subdir in &["characters", "locations", "factions", "secrets"] {
        let src = pack_cards.join(subdir);
        let dst = project_cards.join(subdir);
        if !src.is_dir() || !dst.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&src) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.ends_with(".md") || name_str.starts_with('_') {
                    continue;
                }
                let dst_file = dst.join(&name);
                if dst_file.exists() {
                    conflicts.push(format!("{subdir}/{name_str}"));
                }
            }
        }
    }

    Ok(conflicts)
}

/// Copy all .md files from src to dst. Returns count of files copied.
fn copy_md_files(src: &Path, dst: &Path) -> Result<u32, Box<dyn std::error::Error>> {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('_') {
                    continue;
                }
                let dst_file = dst.join(&name);
                std::fs::copy(&path, &dst_file)?;
                println!(
                    "  {} {}",
                    "✓".green(),
                    dst_file.strip_prefix(dst.ancestors().nth(2).unwrap_or(dst)).unwrap_or(&dst_file).display()
                );
                count += 1;
            }
        }
    }
    Ok(count)
}
