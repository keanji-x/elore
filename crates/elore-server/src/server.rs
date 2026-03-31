//! Axum web server — serves API + static frontend.

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

use crate::graph;

// ══════════════════════════════════════════════════════════════════
// App state
// ══════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct AppState {
    project: Arc<PathBuf>,
}

impl AppState {
    fn entities_dir(&self) -> PathBuf {
        self.project.join(".everlore/entities")
    }
    fn everlore_dir(&self) -> PathBuf {
        self.project.join(".everlore")
    }
}

// ══════════════════════════════════════════════════════════════════
// Response types
// ══════════════════════════════════════════════════════════════════

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

struct AppError(String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0).into_response()
    }
}

// ══════════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════════

async fn get_phases(State(state): State<AppState>) -> Result<Json<PhasesResponse>, AppError> {
    let ps = ProjectState::load(&state.everlore_dir());
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
    AxumPath(phase): AxumPath<String>,
) -> Result<Json<graph::GraphResponse>, AppError> {
    let snapshot = Snapshot::build(&phase, &state.entities_dir(), &state.everlore_dir())
        .map_err(|e| AppError(format!("snapshot build failed: {e}")))?;
    Ok(Json(graph::build_graph(&snapshot)))
}

// ══════════════════════════════════════════════════════════════════
// Server entry
// ══════════════════════════════════════════════════════════════════

#[tokio::main]
pub async fn run(project: PathBuf, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        project: Arc::new(project.clone()),
    };

    // Look for web/ in: 1) project dir, 2) current working dir, 3) next to the binary
    let web_dir = [
        project.join("web"),
        std::env::current_dir().unwrap_or_default().join("web"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or_else(|| {
        format!(
            "web/ directory not found. Looked in:\n  - {}\n  - {}",
            project.join("web").display(),
            std::env::current_dir()
                .unwrap_or_default()
                .join("web")
                .display(),
        )
    })?;

    let app = Router::new()
        .route("/api/phases", get(get_phases))
        .route("/api/graph/{phase}", get(get_graph))
        .fallback_service(ServeDir::new(&web_dir).append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let url = format!("http://localhost:{port}");
    println!("Elore server running at {url}");

    // Auto-open browser
    let _ = open::that(&url);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
