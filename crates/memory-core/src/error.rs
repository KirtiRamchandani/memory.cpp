use thiserror::Error;

pub type Result<T> = std::result::Result<T, MemoryError>;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid memory kind: {0}")]
    InvalidKind(String),

    #[error("invalid vector blob")]
    InvalidVectorBlob,

    #[error("embedding dimensions do not match: expected {expected}, actual {actual}")]
    InvalidEmbeddingDim { expected: usize, actual: usize },

    #[error("embedder error: {0}")]
    Embedder(String),

    #[error("http provider error: {0}")]
    Http(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("sensitive data detected: {0}")]
    SensitiveData(String),
}
