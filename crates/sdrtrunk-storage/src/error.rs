//! Storage layer error types.

use thiserror::Error;

/// Database and storage-related errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database connection failed
    #[error("database connection failed: {0}")]
    Connection(String),

    /// Query execution failed
    #[error("query failed: {0}")]
    Query(String),

    /// Transaction failed
    #[error("transaction failed: {0}")]
    Transaction(String),

    /// Migration failed
    #[error("migration failed: {0}")]
    Migration(String),

    /// Record not found
    #[error("record not found: {entity} with id {id}")]
    NotFound {
        /// Entity type
        entity: String,
        /// Entity id
        id: String,
    },

    /// Constraint violation
    #[error("constraint violation: {constraint}")]
    ConstraintViolation {
        /// Constraint name
        constraint: String,
    },

    /// Serialization failed
    #[error("serialization failed: {0}")]
    Serialization(String),
}

/// Automatic conversion from `sqlx::Error`
impl From<sqlx::Error> for StorageError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::NotFound {
                entity: "unknown".into(),
                id: "unknown".into(),
            },
            sqlx::Error::PoolTimedOut => Self::Connection("pool timeout".into()),
            sqlx::Error::Database(db_err) => Self::Query(db_err.to_string()),
            _ => Self::Query(err.to_string()),
        }
    }
}

/// Bridge conversion so callers using [`sdrtrunk_types::AppError`] can use `?` on `StorageError`.
impl From<StorageError> for sdrtrunk_types::AppError {
    fn from(err: StorageError) -> Self {
        Self::Database(err.to_string())
    }
}

/// Result type alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;
