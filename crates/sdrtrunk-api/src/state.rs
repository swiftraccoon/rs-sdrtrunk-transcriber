//! Application state management

use sdrtrunk_core::{context_error::Result, context_error, Config};
use sdrtrunk_database::PgPool;
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

impl AppState {
    /// Create new application state
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
    pub fn get_storage_path(&self, system_id: &str, date: chrono::NaiveDate) -> PathBuf {
        self.upload_dir
            .join(system_id)
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(date.format("%d").to_string())
    }

    /// Get base upload directory
    pub fn get_upload_dir(&self) -> &PathBuf {
        &self.upload_dir
    }

    /// Check if the application is properly configured
    pub fn validate(&self) -> Result<()> {
        // Check that upload directory exists and is writable
        if !self.upload_dir.exists() {
            return Err(context_error!(
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
