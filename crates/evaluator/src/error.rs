#[derive(Debug, thiserror::Error)]
pub enum EvaluatorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Audit error: {0}")]
    Audit(String),

    #[error("{0}")]
    Other(String),
}
