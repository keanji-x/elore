use std::path::Path;
use colored::Colorize;

use ledger::effect::history::History;
use ledger::input::{entity, goal, secret};

pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore = project.join(".everlore");

    println!("{}", "═══ EverLore Status ═══".cyan().bold());

    if !entities_dir.exists() {
        println!("{}", "项目未初始化 — 运行 elore init".yellow());
        return Ok(());
    }

    // Entities
    let entities = entity::load_entities(&entities_dir).unwrap_or_default();
    let chars: Vec<_> = entities.iter().filter(|e| e.entity_type == "character").collect();
    let locs: Vec<_> = entities.iter().filter(|e| e.entity_type == "location").collect();
    let facs: Vec<_> = entities.iter().filter(|e| e.entity_type == "faction").collect();
    println!("\n实体: {} 角色, {} 地点, {} 势力", chars.len(), locs.len(), facs.len());

    // Secrets
    let secrets = secret::load_secrets(&entities_dir).unwrap_or_default();
    if !secrets.is_empty() {
        println!("秘密: {}", secrets.len());
    }

    // Goals
    let goals = goal::load_goal_entities(&entities_dir).unwrap_or_default();
    if !goals.is_empty() {
        let total: usize = goals.iter().map(|ge| ge.goals.len()).sum();
        println!("目标: {} 角色, {} 目标", goals.len(), total);
    }

    // History
    let history = History::load(&everlore);
    let chapters = history.chapters();
    if !chapters.is_empty() {
        println!("\n章节: {}", chapters.join(", "));
        println!("事件: {} 条", history.entries.len());
    } else {
        println!("\n{}", "无事件历史".dimmed());
    }

    // Drama
    let drama_chapters = resolver::drama::list_drama_chapters(&everlore);
    if !drama_chapters.is_empty() {
        println!("Drama: {}", drama_chapters.join(", "));
    }

    // Drafts
    let drafts = project.join("drafts");
    if drafts.exists() {
        let count = std::fs::read_dir(&drafts)?.count();
        if count > 0 {
            println!("草稿: {} 个文件", count);
        }
    }

    Ok(())
}

pub fn read_chapter(project: &Path, chapter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let draft_path = project.join(format!("drafts/{chapter}.md"));
    if !draft_path.exists() {
        println!("{}", format!("草稿 drafts/{chapter}.md 不存在").yellow());
        println!("  → 先运行 elore write {chapter} 生成 prompt");
        return Ok(());
    }

    let text = std::fs::read_to_string(&draft_path)?;

    // Extract effects
    let effects = executor::extract::extract_effects(&text)?;
    let clean = executor::extract::strip_annotations(&text);

    println!("{}", format!("═══ Read: {chapter} ═══").cyan().bold());
    println!("字数: {}", clean.chars().count());
    println!("提取到 {} 个 effects:", effects.len());
    for op in &effects {
        println!("  • {}", op.describe());
    }

    // Audit
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");
    let snap = ledger::Snapshot::build(chapter, &entities_dir, &everlore_dir)?;
    let report = evaluator::audit::audit_effects(chapter, &snap, &effects, &[]);
    println!("\n{}", report.render());

    Ok(())
}
