use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("local database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("local serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("local media storage error: {0}")]
    Media(String),
    #[error("invalid local action input: {0}")]
    InvalidInput(String),
    #[error("clipboard item was not found")]
    NotFound,
}

pub type AppResult<T> = Result<T, AppError>;
