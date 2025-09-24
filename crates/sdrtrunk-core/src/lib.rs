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
pub mod lazy;
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
            self.source
                .as_ref()
                .map(|e| e.as_ref() as &(dyn StdError + 'static))
        }
    }

    /// Result type alias for context errors
    pub type Result<T> = std::result::Result<T, ContextError>;

    /// Create a context error (like `anyhow::anyhow`!)
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
        ///
        /// # Errors
        ///
        /// Returns a `ContextError` if the original result was an error.
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
            Self::with_context(err, "I/O operation failed")
        }
    }

    impl From<serde_json::Error> for ContextError {
        fn from(err: serde_json::Error) -> Self {
            Self::with_context(err, "JSON serialization failed")
        }
    }

    impl From<config::ConfigError> for ContextError {
        fn from(err: config::ConfigError) -> Self {
            Self::with_context(err, "Configuration error")
        }
    }
}

// Re-export commonly used types
pub use config::Config;
pub use error::{Error, Result};
pub use types::{
    FileData, RadioCall, SystemId, TalkgroupId, TranscriptionConfig, TranscriptionStatus,
};

/// Initialize the logging system
///
/// # Errors
///
/// Returns an error if the logging system cannot be initialized.
pub fn init_logging() -> context_error::Result<()> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_level(true)
                .compact(),
        )
        .init();

    Ok(())
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn test_re_exports() {
        // Test that re-exports work
        let _: Config = Config::default();
        let _: Error = Error::NotFound {
            resource: "test".to_string(),
        };
    }

    #[test]
    fn test_init_logging() {
        // Test logging initialization
        let result = init_logging();
        assert!(result.is_ok());
    }

    #[allow(clippy::missing_panics_doc)]
    mod context_error_tests {
        use super::*;
        use crate::context_error::{ContextError, ResultExt};
        use std::io;

        #[test]
        fn test_context_error_new() {
            let error = ContextError::new("test error");
            assert_eq!(error.to_string(), "test error");
            assert!(error.source().is_none());
        }

        #[test]
        fn test_context_error_with_context() {
            let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
            let error = ContextError::with_context(io_error, "failed to read file");
            assert_eq!(error.to_string(), "failed to read file");
            assert!(error.source().is_some());
        }

        #[test]
        fn test_context_error_display() {
            let error = ContextError::new("display test");
            assert_eq!(format!("{error}"), "display test");
        }

        #[test]
        fn test_context_error_debug() {
            let error = ContextError::new("debug test");
            let debug_str = format!("{error:?}");
            assert!(debug_str.contains("ContextError"));
            assert!(debug_str.contains("debug test"));
        }

        #[test]
        fn test_context_error_macro() {
            let error = context_error!("macro test");
            assert_eq!(error.to_string(), "macro test");

            let error = context_error!("formatted {}", "test");
            assert_eq!(error.to_string(), "formatted test");
        }

        #[test]
        fn test_result_ext_trait() {
            let result: std::result::Result<i32, io::Error> =
                Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            let context_result = result.with_context(|| "operation failed");

            assert!(context_result.is_err());
            let error = context_result.unwrap_err();
            assert_eq!(error.to_string(), "operation failed");
            assert!(error.source().is_some());
        }

        #[test]
        fn test_result_ext_trait_success() {
            let result: std::result::Result<i32, io::Error> = Ok(42);
            let context_result = result.with_context(|| "should not be called");

            assert!(context_result.is_ok());
            assert_eq!(context_result.unwrap(), 42);
        }

        #[test]
        fn test_from_io_error() {
            let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
            let context_error: ContextError = io_error.into();

            assert_eq!(context_error.to_string(), "I/O operation failed");
            assert!(context_error.source().is_some());
        }

        #[test]
        fn test_from_json_error() {
            // Create a JSON error by trying to parse invalid JSON
            let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
            let context_error: ContextError = json_error.into();

            assert_eq!(context_error.to_string(), "JSON serialization failed");
            assert!(context_error.source().is_some());
        }

        #[test]
        fn test_from_config_error() {
            // Create a config error by trying to parse invalid TOML
            let config_error = ::config::Config::builder()
                .add_source(::config::File::from_str(
                    "invalid toml [",
                    ::config::FileFormat::Toml,
                ))
                .build()
                .unwrap_err();
            let context_error: ContextError = config_error.into();

            assert_eq!(context_error.to_string(), "Configuration error");
            assert!(context_error.source().is_some());
        }

        #[test]
        fn test_error_chain() {
            let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
            let context_error = ContextError::with_context(io_error, "failed to read config");

            // Walk the error chain
            let mut current_error = &context_error as &dyn StdError;
            let mut chain = Vec::new();

            while let Some(source) = current_error.source() {
                chain.push(current_error.to_string());
                current_error = source;
            }
            chain.push(current_error.to_string());

            assert_eq!(chain[0], "failed to read config");
            assert_eq!(chain[1], "file not found");
        }

        #[test]
        fn test_context_error_with_empty_message() {
            let error = ContextError::new("");
            assert_eq!(error.to_string(), "");
        }

        #[test]
        fn test_context_error_with_unicode() {
            let error = ContextError::new("测试错误");
            assert_eq!(error.to_string(), "测试错误");
        }

        #[test]
        fn test_nested_context_errors() {
            let inner = ContextError::new("inner error");
            let outer = ContextError::with_context(inner, "outer context");

            assert_eq!(outer.to_string(), "outer context");
            assert!(outer.source().is_some());
            assert_eq!(outer.source().unwrap().to_string(), "inner error");
        }

        #[test]
        fn test_result_type_alias() {
            let success: crate::context_error::Result<i32> = Ok(42);
            assert!(matches!(success, Ok(42)));

            let failure: crate::context_error::Result<i32> = Err(ContextError::new("error"));
            assert!(failure.is_err());
        }
    }
}
