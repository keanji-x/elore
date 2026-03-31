use std::path::PathBuf;

mod graph;
mod server;

fn main() {
    let project = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let port: u16 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    let project = project.canonicalize().unwrap_or(project);

    if let Err(e) = server::run(project, port) {
        eprintln!("\x1b[31mError:\x1b[0m {e}");
        std::process::exit(1);
    }
}
