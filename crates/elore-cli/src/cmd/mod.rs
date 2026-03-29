mod init;
mod snapshot;
mod validate;
mod history;
mod drama;
mod status;
mod plan;
mod diff;
mod whatif;
mod add;
mod read_query;
mod phase;
mod generate;

use crate::Cli;
use read_query::Format;

pub async fn dispatch(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let project = cli.project.canonicalize().unwrap_or(cli.project);

    match cli.command {
        // Project setup
        crate::Command::Init => init::run(&project),
        crate::Command::New { entity_type, id, name } => {
            init::new_entity(&project, &entity_type, &id, name.as_deref())
        }

        // AI write API
        crate::Command::Add { action } => match action {
            crate::AddAction::Entity { json } => add::add_entity(&project, &json),
            crate::AddAction::Drama { json } => add::add_drama(&project, &json),
            crate::AddAction::Secret { json } => add::add_secret(&project, &json),
            crate::AddAction::Effect { chapter, dsl } => {
                history::commit_effect(&project, &chapter, &dsl)
            }
            // v3
            crate::AddAction::Phase { json } => add::add_phase(&project, &json),
            crate::AddAction::Beat { json } => add::add_beat(&project, &json),
            crate::AddAction::Note { json } => add::add_note(&project, &json),
            crate::AddAction::Entities { json } => add::add_entities_batch(&project, &json),
        },

        // AI read API
        crate::Command::Read { action } => match action {
            crate::ReadAction::Snapshot { chapter, format } => {
                read_query::read_snapshot(&project, &chapter, Format::from_str(&format))
            }
            crate::ReadAction::Prompt { chapter, pov } => {
                read_query::read_prompt(&project, &chapter, pov.as_deref())
            }
            crate::ReadAction::Drama { chapter, format } => {
                read_query::read_drama(&project, &chapter, Format::from_str(&format))
            }
            crate::ReadAction::History { chapter, format } => {
                read_query::read_history(&project, chapter.as_deref(), Format::from_str(&format))
            }
            // v3
            crate::ReadAction::Phase { format } => {
                read_query::read_phase(&project, Format::from_str(&format))
            }
            crate::ReadAction::Beats { phase, format } => {
                read_query::read_beats(&project, phase.as_deref(), Format::from_str(&format))
            }
        },

        // Human pipeline
        crate::Command::Snapshot { chapter } => snapshot::run(&project, &chapter).await,
        crate::Command::Validate { chapter } => validate::run(&project, &chapter).await,
        crate::Command::Write { chapter, pov, outline } => {
            snapshot::write_prompt(&project, &chapter, pov.as_deref(), outline.as_deref()).await
        }
        crate::Command::Run { chapter, pov } => {
            snapshot::run_pipeline(&project, &chapter, pov.as_deref()).await
        }
        crate::Command::Plan { chapter } => plan::run(&project, chapter.as_deref()).await,
        crate::Command::Diff { from_chapter, to_chapter } => {
            diff::run(&project, &from_chapter, &to_chapter).await
        }
        crate::Command::Whatif { chapter, effect } => {
            whatif::run(&project, &chapter, &effect).await
        }
        crate::Command::History { action } => history::run(&project, action),
        crate::Command::Drama { action } => drama::run(&project, action),

        crate::Command::Gen { output, phases } => {
            generate::run(&project, output.as_deref(), &phases)
        }

        // v3: phase lifecycle
        crate::Command::Status { format } => {
            phase::status(&project, Format::from_str(&format))
        }
        crate::Command::Checkout { phase_id } => phase::checkout(&project, &phase_id),
        crate::Command::Submit => phase::submit(&project),
        crate::Command::Approve => phase::approve(&project),
        crate::Command::Reject { reason } => phase::reject(&project, &reason),
    }
}
