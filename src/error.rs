use thiserror::Error;

#[derive(Debug, Error)]
pub enum DocAssistError {
    #[error("Failed to analyze codebase: {0}")]
    AnalysisError(String),

    #[error("Failed to parse {language} code: {error}")]
    ParseError { language: String, error: String },

    #[error("Generation error: {0}")]
    GenerationError(String),

    #[error("LLM request failed: {0}")]
    LLMError(String),

    #[error("Rate limit exceeded, retry in {retry_after}s")]
    RateLimitError { retry_after: u64 },

    #[error("Context window exceeded: {tokens} tokens (max: {max})")]
    ContextOverflow { tokens: usize, max: usize },

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("State is stale, codebase has changed. Run without --resume to regenerate.")]
    StaleState,

    #[error("No source files found in the codebase")]
    NoSourceFiles,

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Query execution interrupted by user")]
    Interrupted,

    #[error("Failed to create documentation: {0}")]
    AssemblyError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DocAssistError>;

impl From<anyhow::Error> for DocAssistError {
    fn from(err: anyhow::Error) -> Self {
        DocAssistError::Other(err.to_string())
    }
}

// LiteLLM errors are handled via string conversion in the generator module

impl From<std::io::Error> for DocAssistError {
    fn from(err: std::io::Error) -> Self {
        DocAssistError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for DocAssistError {
    fn from(err: serde_json::Error) -> Self {
        DocAssistError::SerializationError(err.to_string())
    }
}

impl From<toml::de::Error> for DocAssistError {
    fn from(err: toml::de::Error) -> Self {
        DocAssistError::ConfigError(format!("TOML parsing error: {}", err))
    }
}

impl From<ignore::Error> for DocAssistError {
    fn from(err: ignore::Error) -> Self {
        DocAssistError::IoError(format!("File discovery error: {}", err))
    }
}