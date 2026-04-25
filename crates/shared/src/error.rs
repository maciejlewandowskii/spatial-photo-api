use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("authentication failed")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("{0} not found")]
    NotFound(String),

    #[error("insufficient tokens")]
    InsufficientTokens,

    #[error("validation error: {0}")]
    Validation(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("job error: {0}")]
    Job(String),

    #[error("queue error: {0}")]
    Queue(String),
}
