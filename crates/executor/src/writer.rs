//! Writer trait — the pluggable text generation interface.
//!
//! Implementations can be:
//! - `LlmWriter` — calls an LLM API with the AuthorPrompt
//! - `FileWriter` — reads a pre-written chapter from disk
//! - `MockWriter` — for testing

use crate::ExecutorError;
use async_trait::async_trait;

/// Result of a writing operation.
#[derive(Debug, Clone)]
pub struct WriterOutput {
    /// The generated text (may include effect annotations).
    pub text: String,
    /// Model used (or "file" / "mock").
    pub model: String,
    /// Token usage if applicable.
    pub tokens_used: Option<usize>,
}

/// Trait for pluggable text generation backends.
#[async_trait]
pub trait Writer: Send + Sync {
    /// Generate narrative text from a rendered prompt.
    async fn write(&self, prompt: &str, chapter: &str) -> Result<WriterOutput, ExecutorError>;
}

/// A mock writer for testing — returns a fixed output.
pub struct MockWriter {
    pub response: String,
}

#[async_trait]
impl Writer for MockWriter {
    async fn write(&self, _prompt: &str, _chapter: &str) -> Result<WriterOutput, ExecutorError> {
        Ok(WriterOutput {
            text: self.response.clone(),
            model: "mock".into(),
            tokens_used: None,
        })
    }
}

/// A file-based writer — reads pre-written chapter from disk.
pub struct FileWriter {
    pub chapters_dir: std::path::PathBuf,
}

#[async_trait]
impl Writer for FileWriter {
    async fn write(&self, _prompt: &str, chapter: &str) -> Result<WriterOutput, ExecutorError> {
        let path = self.chapters_dir.join(format!("{chapter}.md"));
        let text = std::fs::read_to_string(&path)?;
        Ok(WriterOutput {
            text,
            model: "file".into(),
            tokens_used: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_writer_returns_response() {
        let writer = MockWriter {
            response: "基安走进了沙暴。«effect: move(kian, wasteland)»".into(),
        };
        let output = writer.write("prompt", "ch01").await.unwrap();
        assert_eq!(output.model, "mock");
        assert!(output.text.contains("基安"));
    }
}
