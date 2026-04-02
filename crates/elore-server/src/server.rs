//! Axum web server — multi-project workspace server.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use ledger::state::phase_manager::ProjectState;
use ledger::state::snapshot::Snapshot;

use crate::config::Config;
use crate::graph;

// ══════════════════════════════════════════════════════════════════
// App state
// ══════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct AppState {
    data_dir: Arc<PathBuf>,
}

impl AppState {
    /// Auto-discover projects: subdirectories of data_dir that contain `cards/`.
    fn discover_projects(&self) -> Vec<ProjectInfo> {
        let mut projects = Vec::new();
        let entries = match std::fs::read_dir(self.data_dir.as_ref()) {
            Ok(e) => e,
            Err(_) => return projects,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("cards").is_dir() {
                let id = entry.file_name().to_string_lossy().to_string();
                let everlore = path.join(".everlore");
                let has_build = everlore.is_dir();
                let ps = if has_build {
                    Some(ProjectState::load(&everlore))
                } else {
                    None
                };
                projects.push(ProjectInfo {
                    id,
                    has_build,
                    current_phase: ps.as_ref().and_then(|p| p.current_phase.clone()),
                    phase_count: ps.as_ref().map(|p| p.phases.len()).unwrap_or(0),
                });
            }
        }
        projects.sort_by(|a, b| a.id.cmp(&b.id));
        projects
    }

    /// Resolve a project directory. Returns `None` if the project doesn't exist
    /// or doesn't contain `cards/`.
    fn project_dir(&self, id: &str) -> Option<PathBuf> {
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return None;
        }
        let dir = self.data_dir.join(id);
        if dir.join("cards").is_dir() {
            Some(dir)
        } else {
            None
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Response types
// ══════════════════════════════════════════════════════════════════

#[derive(Serialize)]
struct ProjectInfo {
    id: String,
    has_build: bool,
    current_phase: Option<String>,
    phase_count: usize,
}

#[derive(Serialize)]
struct PhasesResponse {
    current: Option<String>,
    plan: Vec<String>,
    phases: BTreeMap<String, PhaseInfo>,
}

#[derive(Serialize)]
struct PhaseInfo {
    status: String,
    beats: u32,
    words: u32,
    effects: u32,
}

// ══════════════════════════════════════════════════════════════════
// Error type
// ══════════════════════════════════════════════════════════════════

struct AppError(StatusCode, String);

impl AppError {
    fn not_found(msg: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, msg.into())
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

// ══════════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════════

async fn list_projects(State(state): State<AppState>) -> Json<Vec<ProjectInfo>> {
    Json(state.discover_projects())
}

async fn get_phases(
    State(state): State<AppState>,
    AxumPath(project): AxumPath<String>,
) -> Result<Json<PhasesResponse>, AppError> {
    let dir = state
        .project_dir(&project)
        .ok_or_else(|| AppError::not_found(format!("project not found: {project}")))?;
    let everlore = dir.join(".everlore");
    let ps = ProjectState::load(&everlore);
    let phases = ps
        .phases
        .into_iter()
        .map(|(id, entry)| {
            (
                id,
                PhaseInfo {
                    status: format!("{:?}", entry.status).to_lowercase(),
                    beats: entry.beats,
                    words: entry.words,
                    effects: entry.effects,
                },
            )
        })
        .collect();
    Ok(Json(PhasesResponse {
        current: ps.current_phase,
        plan: ps.plan,
        phases,
    }))
}

async fn get_graph(
    State(state): State<AppState>,
    AxumPath((project, phase)): AxumPath<(String, String)>,
) -> Result<Json<graph::GraphResponse>, AppError> {
    let dir = state
        .project_dir(&project)
        .ok_or_else(|| AppError::not_found(format!("project not found: {project}")))?;
    let everlore = dir.join(".everlore");
    let entities = everlore.join("entities");
    let snapshot = Snapshot::build(&phase, &entities, &everlore)
        .map_err(|e| AppError::internal(format!("snapshot build failed: {e}")))?;
    Ok(Json(graph::build_graph(&snapshot)))
}

// ══════════════════════════════════════════════════════════════════
// Server entry
// ══════════════════════════════════════════════════════════════════

#[tokio::main]
pub async fn run(config_path: PathBuf, cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = cfg.resolve_data_dir(&config_path);

    // Create data_dir if it doesn't exist.
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)?;
        println!("Created data directory: {}", data_dir.display());
    }

    // Copy config into data_dir for reference / Docker reuse.
    let data_config = data_dir.join("config.toml");
    std::fs::copy(&config_path, &data_config)?;

    // Discover projects.
    let state = AppState {
        data_dir: Arc::new(data_dir.clone()),
    };
    let projects = state.discover_projects();
    if projects.is_empty() {
        println!("No projects found in {}", data_dir.display());
        println!("Create a project directory with cards/ to get started.");
    } else {
        println!(
            "Found {} project(s): {}",
            projects.len(),
            projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    // Resolve web directory relative to config file location.
    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let cwd = std::env::current_dir().unwrap_or_default();
    let web_dir = [
        config_dir.join("web-next/dist"),
        config_dir.join("web"),
        cwd.join("web-next/dist"),
        cwd.join("web"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or("web directory not found (looked for web-next/dist/ and web/)")?;

    let mut app = Router::new()
        .route("/api/projects", get(list_projects))
        .route("/api/projects/{project}/phases", get(get_phases))
        .route("/api/projects/{project}/graph/{phase}", get(get_graph))
        .fallback_service(ServeDir::new(&web_dir).append_index_html_on_directories(true))
        .with_state(state);

    if cfg.server.cors {
        app = app.layer(CorsLayer::permissive());
    }

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let url = format!("http://{addr}");
    println!("Elore server running at {url}");

    if cfg.server.open_browser {
        let _ = open::that(&url);
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
