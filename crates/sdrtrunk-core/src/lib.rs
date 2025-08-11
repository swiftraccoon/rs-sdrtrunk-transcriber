//! Core types and utilities for `SDRTrunk` transcriber

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    rust_2018_idioms
)]

pub mod config;
pub mod error;
pub mod types;
pub mod utils;

/// Simple error context handling (replacement for anyhow)
pub mod context_error {
    use std::{error::Error as StdError, fmt};

    /// A simple error type that can wrap any error with context
    #[derive(Debug)]
    pub struct ContextError {
        source: Option<Box<dyn StdError + Send + Sync>>,
        message: String,
    }

    impl ContextError {
        /// Create a new context error from a message
        pub fn new<S: Into<String>>(message: S) -> Self {
            Self {
                source: None,
                message: message.into(),
            }
        }

        /// Create a new context error from an existing error with context
        pub fn with_context<E, S>(error: E, message: S) -> Self
        where
            E: StdError + Send + Sync + 'static,
            S: Into<String>,
        {
            Self {
                source: Some(Box::new(error)),
                message: message.into(),
            }
        }
    }

    impl fmt::Display for ContextError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl StdError for ContextError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            self.source.as_ref().map(|e| e.as_ref() as &(dyn StdError + 'static))
        }
    }

    /// Result type alias for context errors
    pub type Result<T> = std::result::Result<T, ContextError>;

    /// Create a context error (like anyhow::anyhow!)
    #[macro_export]
    macro_rules! context_error {
        ($msg:literal) => {
            $crate::context_error::ContextError::new($msg)
        };
        ($fmt:expr, $($arg:tt)*) => {
            $crate::context_error::ContextError::new(format!($fmt, $($arg)*))
        };
    }

    /// Extension trait for adding context to results
    pub trait ResultExt<T> {
        /// Add context to an error
        fn with_context<F, S>(self, f: F) -> Result<T>
        where
            F: FnOnce() -> S,
            S: Into<String>;
    }

    impl<T, E> ResultExt<T> for std::result::Result<T, E>
    where
        E: StdError + Send + Sync + 'static,
    {
        fn with_context<F, S>(self, f: F) -> Result<T>
        where
            F: FnOnce() -> S,
            S: Into<String>,
        {
            self.map_err(|e| ContextError::with_context(e, f()))
        }
    }

    // From implementations for common error types
    impl From<std::io::Error> for ContextError {
        fn from(err: std::io::Error) -> Self {
            ContextError::with_context(err, "I/O operation failed")
        }
    }

    impl From<serde_json::Error> for ContextError {
        fn from(err: serde_json::Error) -> Self {
            ContextError::with_context(err, "JSON serialization failed")
        }
    }

    impl From<config::ConfigError> for ContextError {
        fn from(err: config::ConfigError) -> Self {
            ContextError::with_context(err, "Configuration error")
        }
    }
}

// Re-export commonly used types
pub use config::Config;
pub use error::{Error, Result};
pub use types::{FileData, RadioCall, SystemId, TalkgroupId};

/// Initialize the logging system
///
/// # Errors
/// 
/// Returns an error if the logging system cannot be initialized.
pub fn init_logging() -> context_error::Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    Ok(())
}
