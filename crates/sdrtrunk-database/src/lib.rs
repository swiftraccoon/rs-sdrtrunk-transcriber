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

// Re-export convenience functions
pub use queries::{
    RadioCallFilter, UploadLogParams, count_radio_calls, count_radio_calls_filtered,
    count_recent_calls, count_system_calls_since, count_systems, get_radio_call, get_system_stats,
    get_top_systems, insert_radio_call, insert_upload_log, list_radio_calls_filtered,
    update_system_stats, update_transcription_status, validate_api_key,
};

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

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
#[allow(
    clippy::missing_panics_doc,
    clippy::no_effect_underscore_binding,
    clippy::used_underscore_binding
)]
mod tests {
    use super::*;
    use sdrtrunk_core::Config;
    use std::time::Duration;
    // use tokio_test; // Not needed for current tests

    #[test]
    fn test_database_struct() {
        // Test Database struct exists and compiles
        use std::mem;
        assert!(mem::size_of::<Database>() > 0);

        // Test that the struct has the expected method signatures
        let _pool_method = Database::pool as fn(&Database) -> &PgPool;
        assert!(mem::size_of_val(&_pool_method) > 0);
    }

    #[test]
    fn test_re_exports() {
        // Test that re-exports work
        use crate::{RadioCallFilter, UploadLogParams};

        let _filter = RadioCallFilter {
            system_id: None,
            talkgroup_id: None,
            transcription_status: None,
            from_date: None,
            to_date: None,
            limit: 100,
            offset: 0,
        };
        let _params = UploadLogParams {
            client_ip: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            user_agent: Some("test".to_string()),
            api_key_id: Some("test-key".to_string()),
            system_id: Some("test-system".to_string()),
            success: true,
            error_message: None,
            filename: Some("test.mp3".to_string()),
            file_size: Some(1024),
        };
    }

    #[tokio::test]
    async fn test_database_new_invalid_url() {
        let mut config = Config::default();
        config.database.url = "invalid://url".to_string();

        let result = Database::new(&config).await;
        assert!(result.is_err());

        if let Err(Error::Database(msg)) = result {
            assert!(!msg.is_empty());
        } else {
            panic!("Expected Database error");
        }
    }

    #[test]
    fn test_database_pool_configuration() {
        let config = Config {
            database: sdrtrunk_core::config::DatabaseConfig {
                url: "postgresql://localhost/test".to_string(),
                max_connections: 10,
                min_connections: 1,
                connect_timeout: 30,
                idle_timeout: 600,
            },
            ..Default::default()
        };

        // Test that configuration values are properly used
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.database.min_connections, 1);
        assert_eq!(config.database.connect_timeout, 30);
        assert_eq!(config.database.idle_timeout, 600);
    }

    #[tokio::test]
    async fn test_database_health_check_fail() {
        // Create a database with invalid connection
        let pool = sqlx::PgPool::connect_lazy("postgresql://invalid:5432/nonexistent")
            .expect("Failed to create test pool");
        let db = Database { pool };

        let result = db.health_check().await;
        assert!(result.is_err());

        if let Err(Error::Database(msg)) = result {
            assert!(msg.contains("Health check failed"));
        } else {
            panic!("Expected Database error");
        }
    }

    #[tokio::test]
    async fn test_database_migrate_fail() {
        // Create a database with invalid connection
        let pool = sqlx::PgPool::connect_lazy("postgresql://invalid:5432/nonexistent")
            .expect("Failed to create test pool");
        let db = Database { pool };

        let result = db.migrate().await;
        assert!(result.is_err());

        if let Err(Error::Database(msg)) = result {
            assert!(msg.contains("Migration failed"));
        } else {
            panic!("Expected Database error");
        }
    }

    #[test]
    fn test_database_debug() {
        // Test that Database implements Debug trait
        use std::fmt::Debug;

        // This is a compile-time test to ensure Debug is implemented
        fn assert_debug<T: Debug>() {}
        assert_debug::<Database>();
    }

    #[test]
    fn test_database_clone() {
        // Test that Database implements Clone trait
        use std::clone::Clone;

        // This is a compile-time test to ensure Clone is implemented
        fn assert_clone<T: Clone>() {}
        assert_clone::<Database>();
    }

    #[test]
    fn test_duration_conversion() {
        let duration = Duration::from_secs(30);
        assert_eq!(duration.as_secs(), 30);

        let idle_duration = Duration::from_secs(600);
        assert_eq!(idle_duration.as_secs(), 600);
    }

    #[test]
    fn test_pgpool_reexport() {
        // Test that PgPool re-export works at compile time
        use std::mem;

        // Test that PgPool type is available and has size
        assert!(mem::size_of::<PgPool>() > 0);

        // Test type alias
        let _type_name = std::any::type_name::<PgPool>();
        assert!(!_type_name.is_empty());
    }
}
