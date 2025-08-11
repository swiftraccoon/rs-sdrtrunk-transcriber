//! Error types for the file monitoring service

use std::{error::Error as StdError, fmt, path::PathBuf};

/// Result type alias for monitor operations
pub type Result<T> = std::result::Result<T, MonitorError>;

/// Errors that can occur during file monitoring operations
#[derive(Debug)]
pub enum MonitorError {
    /// File system watcher error
    Watcher {
        /// Error message
        message: String,
    },

    /// File processing error
    Processing {
        /// File path that failed processing
        path: PathBuf,
        /// Error message
        message: String,
    },

    /// Database error
    Database(sqlx::Error),

    /// I/O error
    Io(std::io::Error),

    /// Configuration error
    Configuration {
        /// Error message
        message: String,
    },

    /// Queue error
    Queue {
        /// Error message
        message: String,
    },

    /// Service not running
    ServiceNotRunning,

    /// Service already running
    ServiceAlreadyRunning,

    /// Invalid file format
    InvalidFile {
        /// File path with invalid format
        path: PathBuf,
        /// Reason for invalidity
        reason: String,
    },

    /// File already processed
    FileAlreadyProcessed {
        /// File path that was already processed
        path: PathBuf,
    },

    /// Archive error
    Archive {
        /// File path that failed to archive
        path: PathBuf,
        /// Error message
        message: String,
    },

    /// Timeout error
    Timeout {
        /// Operation that timed out
        operation: String,
    },

    /// Shutdown error
    Shutdown {
        /// Error message
        message: String,
    },
}

impl MonitorError {
    /// Create a new watcher error
    #[must_use]
    pub fn watcher<S: Into<String>>(message: S) -> Self {
        Self::Watcher {
            message: message.into(),
        }
    }

    /// Create a new processing error
    #[must_use]
    pub fn processing<P: Into<PathBuf>, S: Into<String>>(path: P, message: S) -> Self {
        Self::Processing {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a new configuration error
    #[must_use]
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new queue error
    #[must_use]
    pub fn queue<S: Into<String>>(message: S) -> Self {
        Self::Queue {
            message: message.into(),
        }
    }

    /// Create a new invalid file error
    #[must_use]
    pub fn invalid_file<P: Into<PathBuf>, S: Into<String>>(path: P, reason: S) -> Self {
        Self::InvalidFile {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Create a new archive error
    #[must_use]
    pub fn archive<P: Into<PathBuf>, S: Into<String>>(path: P, message: S) -> Self {
        Self::Archive {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a new timeout error
    #[must_use]
    pub fn timeout<S: Into<String>>(operation: S) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create a new shutdown error
    #[must_use]
    pub fn shutdown<S: Into<String>>(message: S) -> Self {
        Self::Shutdown {
            message: message.into(),
        }
    }
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorError::Watcher { message } => write!(f, "File system watcher error: {message}"),
            MonitorError::Processing { path, message } => write!(f, "File processing error for {}: {message}", path.display()),
            MonitorError::Database(err) => write!(f, "Database error: {err}"),
            MonitorError::Io(err) => write!(f, "I/O error: {err}"),
            MonitorError::Configuration { message } => write!(f, "Configuration error: {message}"),
            MonitorError::Queue { message } => write!(f, "Queue error: {message}"),
            MonitorError::ServiceNotRunning => write!(f, "Monitor service is not running"),
            MonitorError::ServiceAlreadyRunning => write!(f, "Monitor service is already running"),
            MonitorError::InvalidFile { path, reason } => write!(f, "Invalid file format for {}: {reason}", path.display()),
            MonitorError::FileAlreadyProcessed { path } => write!(f, "File already processed: {}", path.display()),
            MonitorError::Archive { path, message } => write!(f, "Archive error for {}: {message}", path.display()),
            MonitorError::Timeout { operation } => write!(f, "Operation timed out: {operation}"),
            MonitorError::Shutdown { message } => write!(f, "Shutdown error: {message}"),
        }
    }
}

impl StdError for MonitorError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            MonitorError::Database(err) => Some(err),
            MonitorError::Io(err) => Some(err),
            _ => None,
        }
    }
}

// From implementations for automatic conversions
impl From<sqlx::Error> for MonitorError {
    fn from(err: sqlx::Error) -> Self {
        MonitorError::Database(err)
    }
}

impl From<std::io::Error> for MonitorError {
    fn from(err: std::io::Error) -> Self {
        MonitorError::Io(err)
    }
}
