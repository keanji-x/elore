mod add;
mod build;
mod generate;
mod init;
mod ingest;
pub mod pack;
mod phase;
mod plan;
mod read_query;
mod sync;
// copilot tools
mod lint;
mod suggest;

use crate::Cli;
use read_query::Format;

pub async fn dispatch(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let project = cli.project.canonicalize().unwrap_or(cli.project);

    match cli.command {
        // Project setup
        crate::Command::Init => init::run(&project),
        crate::Command::New {
            entity_type,
            id,
            name,
        } => init::new_entity(&project, &entity_type, &id, name.as_deref()),

        // AI write API
        crate::Command::Add { action } => match action {
            crate::AddAction::Entity { json } => add::add_entity(&project, &json),
            crate::AddAction::Secret { json } => add::add_secret(&project, &json),
            // v3
            crate::AddAction::Phase { json } => add::add_phase(&project, &json),
            crate::AddAction::Beat { json } => add::add_beat(&project, &json),
            crate::AddAction::Note { json } => add::add_note(&project, &json),
            crate::AddAction::Entities { json } => add::add_entities_batch(&project, &json),
        },

        // AI read API
        crate::Command::Read { action } => match action {
            crate::ReadAction::Snapshot { phase, format } => {
                read_query::read_snapshot(&project, &phase, Format::from_str(&format))
            }
            crate::ReadAction::History { phase, format } => {
                read_query::read_history(&project, phase.as_deref(), Format::from_str(&format))
            }
            // v3
            crate::ReadAction::Phase { phase, format } => {
                read_query::read_phase(&project, phase.as_deref(), Format::from_str(&format))
            }
            crate::ReadAction::Beats { phase, format } => {
                read_query::read_beats(&project, phase.as_deref(), Format::from_str(&format))
            }
            crate::ReadAction::PreviousBeat { phase } => {
                read_query::read_previous_beat(&project, phase.as_deref())
            }
        },

        // Human workflow
        crate::Command::Plan { phase } => plan::run(&project, phase.as_deref()).await,

        crate::Command::Gen { output, phases } => {
            generate::run(&project, output.as_deref(), &phases)
        }

        // v3: phase lifecycle
        crate::Command::Status { phase, format } => {
            phase::status(&project, phase.as_deref(), Format::from_str(&format))
        }
        crate::Command::Checkout { phase_id } => phase::checkout(&project, &phase_id),
        crate::Command::Submit => phase::submit(&project),
        crate::Command::Approve => phase::approve(&project),
        crate::Command::Reject { reason } => phase::reject(&project, &reason),

        // v4: file-based workflow
        crate::Command::Build => build::run(&project),
        crate::Command::Ingest => ingest::run(&project),
        crate::Command::Sync => sync::run(&project),

        // v5: AI copilot tools
        crate::Command::LintDrafts => lint::run(&project),
        crate::Command::Suggest => suggest::run(&project).await,

        // v6: pack system
        crate::Command::Pack { action } => match action {
            crate::PackAction::List => pack::list(&project),
            crate::PackAction::Info { name } => pack::info(&project, &name),
            crate::PackAction::Install { name } => pack::install(&project, &name),
        },
    }
}
