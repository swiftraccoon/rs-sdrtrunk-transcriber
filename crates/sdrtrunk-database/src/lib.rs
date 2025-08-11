//! Database models and operations for `SDRTrunk` transcriber

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    rust_2018_idioms
)]

pub mod models;
pub mod queries;

use sdrtrunk_core::{Config, Error, Result};
use sqlx::postgres::PgPoolOptions;

// Re-export PgPool for convenience
pub use sqlx::PgPool;
use std::time::Duration;

/// Database connection pool
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection cannot be established.
    pub async fn new(config: &Config) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .min_connections(config.database.min_connections)
            .acquire_timeout(Duration::from_secs(config.database.connect_timeout))
            .idle_timeout(Duration::from_secs(config.database.idle_timeout))
            .connect(&config.database.url)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run database migrations
    ///
    /// # Errors
    ///
    /// Returns an error if migrations fail to run.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Migration failed: {e}")))?;

        Ok(())
    }

    /// Health check
    ///
    /// # Errors
    ///
    /// Returns an error if the health check fails.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Health check failed: {e}")))?;

        Ok(())
    }
}
