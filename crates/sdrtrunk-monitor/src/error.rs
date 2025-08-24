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
            Self::Watcher { message } => write!(f, "File system watcher error: {message}"),
            Self::Processing { path, message } => {
                write!(f, "File processing error for {}: {message}", path.display())
            }
            Self::Database(err) => write!(f, "Database error: {err}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Configuration { message } => write!(f, "Configuration error: {message}"),
            Self::Queue { message } => write!(f, "Queue error: {message}"),
            Self::ServiceNotRunning => write!(f, "Monitor service is not running"),
            Self::ServiceAlreadyRunning => write!(f, "Monitor service is already running"),
            Self::InvalidFile { path, reason } => {
                write!(f, "Invalid file format for {}: {reason}", path.display())
            }
            Self::FileAlreadyProcessed { path } => {
                write!(f, "File already processed: {}", path.display())
            }
            Self::Archive { path, message } => {
                write!(f, "Archive error for {}: {message}", path.display())
            }
            Self::Timeout { operation } => write!(f, "Operation timed out: {operation}"),
            Self::Shutdown { message } => write!(f, "Shutdown error: {message}"),
        }
    }
}

impl StdError for MonitorError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Database(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

// From implementations for automatic conversions
impl From<sqlx::Error> for MonitorError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<std::io::Error> for MonitorError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_watcher_error() {
        let error = MonitorError::watcher("Watch failed");
        assert!(matches!(error, MonitorError::Watcher { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("File system watcher error"));
        assert!(display_str.contains("Watch failed"));
    }

    #[test]
    fn test_processing_error() {
        let path = PathBuf::from("/tmp/test.mp3");
        let error = MonitorError::processing(&path, "Processing failed");
        assert!(matches!(error, MonitorError::Processing { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("File processing error"));
        assert!(display_str.contains("test.mp3"));
        assert!(display_str.contains("Processing failed"));
    }

    #[test]
    fn test_database_error() {
        let db_error = sqlx::Error::RowNotFound;
        let error = MonitorError::Database(db_error);
        assert!(matches!(error, MonitorError::Database(_)));
        let display_str = format!("{error}");
        assert!(display_str.contains("Database error"));
        assert!(display_str.contains("no rows returned"));
    }

    #[test]
    fn test_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let error = MonitorError::Io(io_error);
        assert!(matches!(error, MonitorError::Io(_)));
        let display_str = format!("{error}");
        assert!(display_str.contains("I/O error"));
        assert!(display_str.contains("File not found"));
    }

    #[test]
    fn test_configuration_error() {
        let error = MonitorError::configuration("Invalid config");
        assert!(matches!(error, MonitorError::Configuration { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Configuration error"));
        assert!(display_str.contains("Invalid config"));
    }

    #[test]
    fn test_queue_error() {
        let error = MonitorError::queue("Queue overflow");
        assert!(matches!(error, MonitorError::Queue { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Queue error"));
        assert!(display_str.contains("Queue overflow"));
    }

    #[test]
    fn test_service_errors() {
        let not_running = MonitorError::ServiceNotRunning;
        assert!(matches!(not_running, MonitorError::ServiceNotRunning));
        let display_str = format!("{not_running}");
        assert!(display_str.contains("Monitor service is not running"));

        let already_running = MonitorError::ServiceAlreadyRunning;
        assert!(matches!(
            already_running,
            MonitorError::ServiceAlreadyRunning
        ));
        let display_str = format!("{already_running}");
        assert!(display_str.contains("Monitor service is already running"));
    }

    #[test]
    fn test_invalid_file_error() {
        let path = PathBuf::from("/tmp/invalid.txt");
        let error = MonitorError::invalid_file(&path, "Not an MP3 file");
        assert!(matches!(error, MonitorError::InvalidFile { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Invalid file format"));
        assert!(display_str.contains("invalid.txt"));
        assert!(display_str.contains("Not an MP3 file"));
    }

    #[test]
    fn test_file_already_processed_error() {
        let path = PathBuf::from("/tmp/processed.mp3");
        let error = MonitorError::FileAlreadyProcessed { path: path };
        assert!(matches!(error, MonitorError::FileAlreadyProcessed { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("File already processed"));
        assert!(display_str.contains("processed.mp3"));
    }

    #[test]
    fn test_archive_error() {
        let path = PathBuf::from("/tmp/archive.mp3");
        let error = MonitorError::archive(&path, "Archive failed");
        assert!(matches!(error, MonitorError::Archive { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Archive error"));
        assert!(display_str.contains("archive.mp3"));
        assert!(display_str.contains("Archive failed"));
    }

    #[test]
    fn test_timeout_error() {
        let error = MonitorError::timeout("file processing");
        assert!(matches!(error, MonitorError::Timeout { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Operation timed out"));
        assert!(display_str.contains("file processing"));
    }

    #[test]
    fn test_shutdown_error() {
        let error = MonitorError::shutdown("Graceful shutdown failed");
        assert!(matches!(error, MonitorError::Shutdown { .. }));
        let display_str = format!("{error}");
        assert!(display_str.contains("Shutdown error"));
        assert!(display_str.contains("Graceful shutdown failed"));
    }

    #[test]
    fn test_error_source() {
        // Test errors that have a source
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied");
        let monitor_io_error = MonitorError::Io(io_error);
        assert!(monitor_io_error.source().is_some());

        let db_error = MonitorError::Database(sqlx::Error::RowNotFound);
        assert!(db_error.source().is_some());

        // Test errors that don't have a source
        let watcher_error = MonitorError::watcher("Test");
        assert!(watcher_error.source().is_none());

        let service_error = MonitorError::ServiceNotRunning;
        assert!(service_error.source().is_none());
    }

    #[test]
    fn test_from_conversions() {
        // Test From<sqlx::Error>
        let db_error = sqlx::Error::RowNotFound;
        let monitor_error: MonitorError = db_error.into();
        assert!(matches!(monitor_error, MonitorError::Database(_)));

        // Test From<std::io::Error>
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let monitor_error: MonitorError = io_error.into();
        assert!(matches!(monitor_error, MonitorError::Io(_)));
    }

    #[test]
    fn test_result_type_alias() {
        fn test_function() -> Result<i32> {
            Ok(42)
        }

        let result = test_function();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        fn error_function() -> Result<i32> {
            Err(MonitorError::ServiceNotRunning)
        }

        let result = error_function();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MonitorError::ServiceNotRunning
        ));
    }

    #[test]
    fn test_error_debug() {
        let error = MonitorError::watcher("Debug test");
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("Watcher"));
        assert!(debug_str.contains("Debug test"));

        let processing_error = MonitorError::processing("/tmp/debug.mp3", "Debug processing");
        let debug_str = format!("{processing_error:?}");
        assert!(debug_str.contains("Processing"));
        assert!(debug_str.contains("debug.mp3"));
    }

    #[test]
    fn test_error_constructor_variations() {
        // Test with String
        let error1 = MonitorError::watcher(String::from("String message"));
        assert!(matches!(error1, MonitorError::Watcher { .. }));

        // Test with &str
        let error2 = MonitorError::watcher("str message");
        assert!(matches!(error2, MonitorError::Watcher { .. }));

        // Test path constructors with different types
        let path_buf = PathBuf::from("/test/path");
        let error3 = MonitorError::processing(&path_buf, "PathBuf message");
        assert!(matches!(error3, MonitorError::Processing { .. }));

        let path_str = "/test/path2";
        let error4 = MonitorError::processing(path_str, "str path");
        assert!(matches!(error4, MonitorError::Processing { .. }));
    }

    #[test]
    fn test_error_chain() {
        // Test error chaining with source errors
        let io_error = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Pipe broken");
        let monitor_error = MonitorError::Io(io_error);

        let mut source_chain = Vec::new();
        let mut current_error: &dyn StdError = &monitor_error;

        while let Some(source) = current_error.source() {
            source_chain.push(source.to_string());
            current_error = source;
        }

        assert!(!source_chain.is_empty());
        assert!(source_chain[0].contains("Pipe broken"));
    }

    #[test]
    fn test_error_display_all_variants() {
        let test_cases = vec![
            MonitorError::watcher("watcher test"),
            MonitorError::processing("/test/file.mp3", "processing test"),
            MonitorError::Database(sqlx::Error::RowNotFound),
            MonitorError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "io test")),
            MonitorError::configuration("config test"),
            MonitorError::queue("queue test"),
            MonitorError::ServiceNotRunning,
            MonitorError::ServiceAlreadyRunning,
            MonitorError::invalid_file("/test/invalid.txt", "invalid test"),
            MonitorError::FileAlreadyProcessed {
                path: PathBuf::from("/test/processed.mp3"),
            },
            MonitorError::archive("/test/archive.mp3", "archive test"),
            MonitorError::timeout("timeout test"),
            MonitorError::shutdown("shutdown test"),
        ];

        for error in test_cases {
            let display_str = format!("{error}");
            assert!(
                !display_str.is_empty(),
                "Error display should not be empty for: {error:?}"
            );

            // Check that each error type has appropriate keywords in display
            match error {
                MonitorError::Watcher { .. } => assert!(display_str.contains("watcher")),
                MonitorError::Processing { .. } => assert!(display_str.contains("processing")),
                MonitorError::Database(_) => assert!(display_str.contains("Database")),
                MonitorError::Io(_) => assert!(display_str.contains("I/O")),
                MonitorError::Configuration { .. } => {
                    assert!(display_str.contains("Configuration"));
                }
                MonitorError::Queue { .. } => assert!(display_str.contains("Queue")),
                MonitorError::ServiceNotRunning => assert!(display_str.contains("not running")),
                MonitorError::ServiceAlreadyRunning => {
                    assert!(display_str.contains("already running"));
                }
                MonitorError::InvalidFile { .. } => assert!(display_str.contains("Invalid file")),
                MonitorError::FileAlreadyProcessed { .. } => {
                    assert!(display_str.contains("already processed"));
                }
                MonitorError::Archive { .. } => assert!(display_str.contains("Archive")),
                MonitorError::Timeout { .. } => assert!(display_str.contains("timed out")),
                MonitorError::Shutdown { .. } => assert!(display_str.contains("Shutdown")),
            }
        }
    }
}
