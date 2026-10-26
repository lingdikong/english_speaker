use thiserror::Error;

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("tts engine error: {0}")]
    Engine(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("translation error: {0}")]
    Translation(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("cancelled")]
    Cancelled,
    #[error("internal error: {0}")]
    Internal(String),
}
