//! Error types used across the bmo library.

use thiserror::Error;

/// Top-level error type for bmo operations.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum BmoError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error: {0}")]
    Db(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Machine-readable category for a bmo error, used to select a CLI exit code.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    /// Uncategorized or internal error.
    General,
    /// The requested resource does not exist.
    NotFound,
    /// Input failed validation.
    Validation,
    /// The operation conflicts with existing data.
    Conflict,
}

#[allow(dead_code)]
impl ErrorCode {
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorCode::General => 1,
            ErrorCode::NotFound => 2,
            ErrorCode::Validation => 3,
            ErrorCode::Conflict => 4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::General => "general",
            ErrorCode::NotFound => "not-found",
            ErrorCode::Validation => "validation",
            ErrorCode::Conflict => "conflict",
        }
    }
}

impl From<&BmoError> for ErrorCode {
    fn from(e: &BmoError) -> Self {
        match e {
            BmoError::NotFound(_) => ErrorCode::NotFound,
            BmoError::Validation(_) => ErrorCode::Validation,
            BmoError::Conflict(_) => ErrorCode::Conflict,
            BmoError::Db(_) => ErrorCode::General,
            BmoError::Io(_) => ErrorCode::General,
        }
    }
}
