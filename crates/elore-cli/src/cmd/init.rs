use std::path::Path;
use colored::Colorize;

/// Initialize a new EverLore project.
pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let entities = everlore.join("entities");
    let drama = everlore.join("drama");
    let phases = everlore.join("phases");
    let beats = everlore.join("beats");
    let annotations = everlore.join("annotations");
    let drafts = project.join("drafts");

    std::fs::create_dir_all(&entities)?;
    std::fs::create_dir_all(&drama)?;
    std::fs::create_dir_all(&phases)?;
    std::fs::create_dir_all(&beats)?;
    std::fs::create_dir_all(&annotations)?;
    std::fs::create_dir_all(&drafts)?;

    // Create empty secrets.yaml
    let secrets_path = entities.join("secrets.yaml");
    if !secrets_path.exists() {
        std::fs::write(&secrets_path, "secrets: []\n")?;
    }

    println!("{}", "✓ EverLore v3 项目已初始化".green().bold());
    println!("  {}", entities.display());
    println!("  {}", drama.display());
    println!("  {} (v3)", phases.display());
    println!("  {} (v3)", beats.display());
    println!("  {} (v3)", annotations.display());
    println!("  {}", drafts.display());
    Ok(())
}

/// Create a new entity scaffold.
pub fn new_entity(
    project: &Path,
    entity_type: &str,
    id: &str,
    name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = project.join(".everlore/entities");
    std::fs::create_dir_all(&entities_dir)?;

    let path = entities_dir.join(format!("{id}.json"));
    if path.exists() {
        return Err(format!("实体 {id} 已存在: {}", path.display()).into());
    }

    let display_name = name.unwrap_or(id);
    let json = match entity_type {
        "character" => serde_json::json!({
            "type": "character",
            "id": id,
            "name": display_name,
            "traits": [],
            "beliefs": [],
            "desires": [],
            "intentions": [],
            "location": null,
            "relationships": [],
            "inventory": [],
            "tags": ["active"]
        }),
        "location" => serde_json::json!({
            "type": "location",
            "id": id,
            "name": display_name,
            "properties": [],
            "connections": [],
            "tags": ["active"]
        }),
        "faction" => serde_json::json!({
            "type": "faction",
            "id": id,
            "name": display_name,
            "alignment": null,
            "members": [],
            "rivals": [],
            "tags": ["active"]
        }),
        _ => return Err(format!("未知类型: {entity_type} (可选: character, location, faction)").into()),
    };

    let content = serde_json::to_string_pretty(&json)?;
    std::fs::write(&path, content)?;

    println!("{} {} {}", "✓".green().bold(), entity_type.cyan(), id.bold());
    println!("  → {}", path.display());
    Ok(())
}
