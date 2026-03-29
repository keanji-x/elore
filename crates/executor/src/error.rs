#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Effect extraction error: {0}")]
    Extraction(String),

    #[error("Writer error: {0}")]
    Writer(String),

    #[error("{0}")]
    Other(String),
}
