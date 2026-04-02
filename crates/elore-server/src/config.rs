//! Server configuration loaded from `elore.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: String,
    pub open_browser: bool,
    pub cors: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 3000,
            data_dir: ".data".into(),
            open_browser: true,
            cors: true,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!(
                        "\x1b[33mwarning:\x1b[0m failed to parse {}: {e}",
                        path.display()
                    );
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    /// Resolve data_dir relative to the config file's parent.
    pub fn resolve_data_dir(&self, config_path: &Path) -> PathBuf {
        let p = PathBuf::from(&self.server.data_dir);
        if p.is_absolute() {
            p
        } else {
            let parent = config_path.parent().unwrap_or(Path::new("."));
            parent.join(p)
        }
    }
}
