//! Ledger error types.

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Effect parse error: {0}")]
    EffectParse(String),

    #[error("Reasoning error: {0}")]
    Reasoning(String),

    #[error("Snapshot error: {0}")]
    Snapshot(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("{0}")]
    Other(String),
}
