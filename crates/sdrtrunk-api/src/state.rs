//! Application state management

use anyhow::{Result, anyhow};
use sdrtrunk_protocol::Config;
use sdrtrunk_storage::PgPool;
use std::path::PathBuf;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: Config,
    /// Database connection pool
    pub pool: PgPool,
    /// Base directory for uploaded files
    pub upload_dir: PathBuf,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("pool", &"PgPool { .. }")
            .field("upload_dir", &self.upload_dir)
            .finish()
    }
}

impl AppState {
    /// Create new application state
    ///
    /// # Errors
    ///
    /// Returns an error if the upload directory cannot be created.
    pub fn new(config: Config, pool: PgPool) -> Result<Self> {
        // Build the full upload directory path
        let upload_dir = config.storage.base_dir.join(&config.storage.upload_dir);

        // Ensure upload directory exists
        std::fs::create_dir_all(&upload_dir)?;

        Ok(Self {
            config,
            pool,
            upload_dir,
        })
    }

    /// Get file storage path for a given system and date
    #[must_use]
    pub fn get_storage_path(&self, system_id: &str, date: chrono::NaiveDate) -> PathBuf {
        self.upload_dir
            .join(system_id)
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(date.format("%d").to_string())
    }

    /// Get base upload directory
    #[must_use]
    pub const fn get_upload_dir(&self) -> &PathBuf {
        &self.upload_dir
    }

    /// Check if the application is properly configured
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self) -> Result<()> {
        // Check that upload directory exists and is writable
        if !self.upload_dir.exists() {
            return Err(anyhow!(
                "Upload directory does not exist: {}",
                self.upload_dir.display()
            ));
        }

        // Try to create a test file to verify write permissions
        let test_file = self.upload_dir.join(".write_test");
        std::fs::write(&test_file, "test")?;
        std::fs::remove_file(&test_file)?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::redundant_clone,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args,
    unused_qualifications,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::redundant_closure_for_method_calls,
    clippy::fn_params_excessive_bools,
    clippy::similar_names,
    clippy::map_unwrap_or,
    clippy::unused_async,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::manual_string_new,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::single_char_pattern,
    clippy::ip_constant,
    clippy::or_fun_call,
    clippy::cast_lossless,
    clippy::needless_collect,
    clippy::single_match_else,
    clippy::needless_raw_string_hashes,
    clippy::match_same_arms
)]
mod tests {
    use super::*;
    use sdrtrunk_storage::PgPool;
    use tempfile::TempDir;

    #[test]
    fn test_appstate_basics() {
        // Just test that the AppState struct exists and compiles
        use std::mem;
        assert!(mem::size_of::<AppState>() > 0);

        // Test path generation logic without database
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().join("uploads");

        let test_date = chrono::NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        let storage_path = base_path
            .join("police_system")
            .join(test_date.format("%Y").to_string())
            .join(test_date.format("%m").to_string())
            .join(test_date.format("%d").to_string());

        let expected = base_path
            .join("police_system")
            .join("2023")
            .join("12")
            .join("25");

        assert_eq!(storage_path, expected);
    }

    #[test]
    fn test_directory_operations() {
        // Test basic directory operations used by AppState
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_path = temp_dir.path().join("uploads");
        std::fs::create_dir_all(&upload_path).expect("Failed to create upload dir");

        // Test that directory exists
        assert!(upload_path.exists());

        // Test write permissions
        let test_file = upload_path.join(".write_test");
        let write_result = std::fs::write(&test_file, "test");
        assert!(write_result.is_ok());

        let remove_result = std::fs::remove_file(&test_file);
        assert!(remove_result.is_ok());
    }

    fn create_test_config(upload_dir: PathBuf) -> Config {
        let mut config = Config::default();
        config.storage.base_dir = upload_dir.parent().unwrap().to_path_buf();
        config.storage.upload_dir = upload_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        config
    }

    fn create_test_pool() -> PgPool {
        // Create a dummy pool for testing - we don't actually need a real database
        use sqlx::postgres::PgPoolOptions;
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost/test")
            .expect("Failed to create test pool")
    }

    #[tokio::test]
    async fn test_appstate_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state = AppState::new(config.clone(), pool.clone()).expect("Failed to create AppState");

        assert!(upload_dir.exists());
        assert_eq!(state.upload_dir, upload_dir);
        assert_eq!(state.config.storage.base_dir, config.storage.base_dir);
    }

    #[tokio::test]
    async fn test_get_storage_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state = AppState::new(config, pool).expect("Failed to create AppState");

        let test_date = chrono::NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        let storage_path = state.get_storage_path("police_system", test_date);

        let expected = upload_dir
            .join("police_system")
            .join("2023")
            .join("12")
            .join("25");

        assert_eq!(storage_path, expected);
    }

    #[tokio::test]
    async fn test_get_upload_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state = AppState::new(config, pool).expect("Failed to create AppState");

        assert_eq!(state.get_upload_dir(), &upload_dir);
    }

    #[tokio::test]
    async fn test_validate_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state = AppState::new(config, pool).expect("Failed to create AppState");

        // Should succeed since directory exists and is writable
        assert!(state.validate().is_ok());
    }

    #[tokio::test]
    async fn test_validate_nonexistent_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state = AppState::new(config, pool).expect("Failed to create AppState");

        // Remove the directory after creation
        std::fs::remove_dir_all(&state.upload_dir).expect("Failed to remove dir");

        // Validation should fail
        let result = state.validate();
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("does not exist"));
    }

    #[tokio::test]
    async fn test_appstate_clone() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let upload_dir = temp_dir.path().join("uploads");
        let config = create_test_config(upload_dir.clone());
        let pool = create_test_pool();

        let state1 = AppState::new(config, pool).expect("Failed to create AppState");
        let state2 = state1.clone();

        assert_eq!(state1.upload_dir, state2.upload_dir);
        assert_eq!(
            state1.config.storage.base_dir,
            state2.config.storage.base_dir
        );
    }
}
