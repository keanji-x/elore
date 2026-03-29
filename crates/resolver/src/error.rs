//! Resolver error types.

#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Prompt build error: {0}")]
    PromptBuild(String),

    #[error("Drama node not found for chapter: {0}")]
    DramaNotFound(String),

    #[error("{0}")]
    Other(String),
}
