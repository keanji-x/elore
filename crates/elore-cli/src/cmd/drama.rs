use std::path::Path;

use resolver::drama as drama_mod;

pub fn run(project: &Path, action: crate::DramaAction) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");

    match action {
        crate::DramaAction::Show { chapter } => {
            let node = drama_mod::load_drama(&everlore, &chapter)?;
            println!("{}", node.render());
            Ok(())
        }
    }
}
