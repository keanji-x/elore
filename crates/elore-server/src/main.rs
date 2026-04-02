use std::path::PathBuf;

mod config;
mod graph;
mod server;

fn main() {
    let config_path = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("\x1b[31mError:\x1b[0m missing config path");
            eprintln!("Usage: elore-server <config.toml> [port]");
            std::process::exit(1);
        }
    };

    if !config_path.exists() {
        eprintln!(
            "\x1b[31mError:\x1b[0m config not found: {}",
            config_path.display()
        );
        std::process::exit(1);
    }

    let config_path = config_path.canonicalize().unwrap_or(config_path);
    let mut cfg = config::Config::load(&config_path);

    if let Some(port) = std::env::args().nth(2).and_then(|s| s.parse().ok()) {
        cfg.server.port = port;
    }

    if let Err(e) = server::run(config_path, cfg) {
        eprintln!("\x1b[31mError:\x1b[0m {e}");
        std::process::exit(1);
    }
}
