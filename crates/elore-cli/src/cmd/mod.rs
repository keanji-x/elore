mod add;
mod build;
mod content;
mod init;
pub mod pack;
mod publish;
mod suggest;

use crate::Cli;

pub async fn dispatch(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let project = cli.project.canonicalize().unwrap_or(cli.project);

    match cli.command {
        // Project
        crate::Command::Init => init::run(&project),
        crate::Command::New {
            entity_type,
            id,
            name,
        } => init::new_entity(&project, &entity_type, &id, name.as_deref()),
        crate::Command::Build => build::run(&project),

        // Content tree (top-level)
        crate::Command::Tree { format } => {
            content::list(&project, Format::from_str(&format))
        }
        crate::Command::Show { id, format } => {
            let id = resolve_active(&project, id)?;
            content::show(&project, &id, Format::from_str(&format))
        }
        crate::Command::Activate { id } => content::activate(&project, &id),
        crate::Command::Edit { id } => content::edit(&project, &id),
        crate::Command::Commit { id } => {
            let id = resolve_active(&project, id)?;
            content::commit(&project, &id)
        }
        crate::Command::Diff { id } => {
            let id = resolve_active(&project, id)?;
            content::diff(&project, &id)
        }
        crate::Command::Snapshot { id, format, before } => {
            let id = resolve_active(&project, id)?;
            content::snapshot(&project, &id, Format::from_str(&format), before)
        }

        // Read (aggregated views)
        crate::Command::Read { mode } => match mode {
            crate::ReadMode::Level { depth } => content::read_level(&project, depth),
            crate::ReadMode::Leaf { id } => {
                let id = resolve_active(&project, id)?;
                content::read_leaf(&project, &id)
            }
            crate::ReadMode::Path { id } => {
                let id = resolve_active(&project, id)?;
                content::read_path(&project, &id)
            }
            crate::ReadMode::Parent { id } => {
                let id = resolve_active(&project, id)?;
                content::read_parent(&project, &id)
            }
            crate::ReadMode::Sibling { id } => {
                let id = resolve_active(&project, id)?;
                content::read_sibling(&project, &id)
            }
            crate::ReadMode::Pov { id, who } => {
                let id = resolve_active(&project, id)?;
                content::read_pov(&project, &id, who.as_deref())
            }
        },

        // Context — if no id given, use main_role from active content node
        crate::Command::Context { id, full } => {
            let entity_id = match id {
                Some(eid) => eid,
                None => {
                    // Resolve main_role from active content node
                    let everlore = project.join(".everlore");
                    let cards_dir = project.join("cards");
                    let tree = ledger::ContentTree::load(&everlore);
                    let active = tree.active.as_deref().or(tree.root.as_deref())
                        .ok_or("没有活跃节点")?;
                    let contents = ledger::card::load_content_cards(&cards_dir)?;
                    let content_map: std::collections::BTreeMap<String, ledger::Content> =
                        contents.into_iter().map(|c| (c.id.clone(), c)).collect();
                    ledger::effective_main_role(active, &tree, &content_map)
                        .ok_or("没有 main_role — 请指定实体 ID 或在 card 中设置 main_role")?
                }
            };
            content::context(&project, &entity_id, full)
        }

        // Output
        crate::Command::Publish { output } => {
            publish::run(&project, output.as_deref())
        }

        // AI API
        crate::Command::Add { action } => match action {
            crate::AddAction::Entity { json } => add::add_entity(&project, &json),
            crate::AddAction::Secret { json } => add::add_secret(&project, &json),
            crate::AddAction::Entities { json } => add::add_entities_batch(&project, &json),
        },
        crate::Command::Suggest => suggest::run(&project).await,

        // Pack
        crate::Command::Pack { action } => match action {
            crate::PackAction::List => pack::list(&project),
            crate::PackAction::Info { name } => pack::info(&project, &name),
            crate::PackAction::Install { name } => pack::install(&project, &name),
        },

        // Memory harness
        crate::Command::Harness {
            novel,
            title,
            roster,
            discover,
        } => {
            let report = ledger::memory::harness::run(
                &novel,
                &title,
                roster.as_deref(),
                discover,
            )?;
            report.print_summary();
            Ok(())
        }
    }
}

/// Resolve an optional content id: use the given id, or fall back to the active node.
fn resolve_active(
    project: &std::path::Path,
    id: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(id) = id {
        return Ok(id);
    }
    let everlore = project.join(".everlore");
    let tree = ledger::ContentTree::load(&everlore);
    tree.active
        .ok_or_else(|| "没有指定 id，也没有 active 节点 — 请先 `elore activate <id>`".into())
}

/// Output format for structured commands.
pub enum Format {
    Human,
    Json,
}

impl Format {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            _ => Self::Human,
        }
    }
}
