use colored::Colorize;
use std::path::Path;

use ledger::state::graph::WorldGraph;
use ledger::state::snapshot::Snapshot;

use ledger::effect::history::History;
use resolver::drama;
use resolver::prompt::AuthorPrompt;
use resolver::validate;

/// Build and display a world snapshot.
pub async fn run(project: &Path, chapter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let snap = build_snapshot(project, chapter)?;

    println!("{}", format!("═══ Snapshot: {chapter} ═══").cyan().bold());
    println!();
    println!("角色 ({}):", snap.characters().len());
    for c in snap.characters() {
        let name = c.name.as_deref().unwrap_or(&c.id);
        let loc = c.location.as_deref().unwrap_or("?");
        println!("  {} ({}) @ {}", name.bold(), c.id, loc);
        if !c.traits.is_empty() {
            println!("    特质: {}", c.traits.join(", "));
        }
        if !c.inventory.is_empty() {
            println!("    物品: {}", c.inventory.join(", "));
        }
    }

    println!("\n地点 ({}):", snap.locations().len());
    for l in snap.locations() {
        let name = l.name.as_deref().unwrap_or(&l.id);
        println!("  {} ({})", name.bold(), l.id);
    }

    if !snap.secrets.is_empty() {
        println!("\n秘密 ({}):", snap.secrets.len());
        for s in &snap.secrets {
            let tech = s.classify();
            println!("  {} — {:?}", s.id.bold(), tech);
        }
    }

    let graph = WorldGraph::build(&snap.entities);
    println!(
        "\n图索引: {} 节点, {} 边",
        graph.node_count(),
        graph.edge_count()
    );

    Ok(())
}

/// Write prompt for a chapter.
pub async fn write_prompt(
    project: &Path,
    chapter: &str,
    pov: Option<&str>,
    outline: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let snap = build_snapshot(project, chapter)?;
    let everlore = project.join(".everlore");
    let mut drama_node = drama::load_drama(&everlore, chapter)?;

    // Override POV if specified on command line
    if let Some(p) = pov {
        drama_node.director_notes.pov = Some(p.to_string());
    }

    let history = History::load(&everlore);
    let prev_summary = History::previous_chapter_summary(&history, chapter);
    let outline_text = if let Some(p) = outline {
        Some(std::fs::read_to_string(p)?)
    } else {
        None
    };

    let reasoning = ledger::Program::from_snapshot(&snap, None).run().await?;
    let prompt = AuthorPrompt::build(
        &snap,
        &drama_node,
        Some(&reasoning),
        prev_summary.as_deref(),
        outline_text.as_deref(),
    );

    // Write prompt to drafts/
    let drafts = project.join("drafts");
    std::fs::create_dir_all(&drafts)?;
    let prompt_path = drafts.join(format!("{chapter}_prompt.md"));
    std::fs::write(&prompt_path, &prompt.rendered)?;

    println!(
        "{}",
        format!("✓ Prompt 已生成: {}", prompt_path.display())
            .green()
            .bold()
    );
    println!("  hash: {}", &prompt.content_hash[..16]);
    if let Some(p) = &prompt.pov {
        println!("  POV: {}", p.cyan());
    }
    println!("  长度: {} 字符", prompt.rendered.len());
    Ok(())
}

/// Run the full pipeline for a chapter.
pub async fn run_pipeline(
    project: &Path,
    chapter: &str,
    pov: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("═══ elore run {chapter} ═══").cyan().bold());

    // Phase 1: Snapshot
    println!("\n{}", "Phase 1: Engine (Snapshot)".yellow().bold());
    let snap = build_snapshot(project, chapter)?;
    println!(
        "  ✓ {} 实体, {} 秘密",
        snap.entities.len(),
        snap.secrets.len()
    );

    // Phase 2: Validate + Prompt
    println!(
        "\n{}",
        "Phase 2: Director (Validate + Prompt)".yellow().bold()
    );
    let everlore = project.join(".everlore");
    let mut drama_node = drama::load_drama(&everlore, chapter)?;
    if let Some(p) = pov {
        drama_node.director_notes.pov = Some(p.to_string());
    }
    let reasoning = ledger::Program::from_snapshot(&snap, None).run().await?;
    let verdict = validate::validate(&snap, &drama_node, Some(&reasoning));
    println!("  {}", verdict.render());

    let history = History::load(&everlore);
    let prev_summary = History::previous_chapter_summary(&history, chapter);
    let prompt = AuthorPrompt::build(&snap, &drama_node, Some(&reasoning), prev_summary.as_deref(), None);

    let drafts = project.join("drafts");
    std::fs::create_dir_all(&drafts)?;
    let prompt_path = drafts.join(format!("{chapter}_prompt.md"));
    std::fs::write(&prompt_path, &prompt.rendered)?;
    println!("  ✓ Prompt → {}", prompt_path.display());

    // Phase 3 & 4 would invoke an LLM — for now, print instructions
    println!("\n{}", "Phase 3: Author (写作)".yellow().bold());
    println!("  → 使用上面生成的 prompt 调用 LLM, 或手写章节");
    println!("  → 在文本中用 «effect: op(args)» 标注状态变化");
    println!("  → 保存到 drafts/{chapter}.md");

    println!("\n{}", "Phase 4: Reader (审核)".yellow().bold());
    println!("  → 运行 elore read {chapter} 进行一致性审核");
    println!("  → 运行 elore history add {chapter} \"effect_dsl\" 提交 effects");

    Ok(())
}

fn build_snapshot(project: &Path, chapter: &str) -> Result<Snapshot, Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    let everlore_dir = project.join(".everlore");

    if !entities_dir.exists() {
        return Err("项目未初始化, 请先运行 elore init".into());
    }

    Snapshot::build(chapter, &entities_dir, &everlore_dir).map_err(|e| e.into())
}
