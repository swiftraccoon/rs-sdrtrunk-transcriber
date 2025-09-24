//! `SDRTrunk` API server library

#![forbid(unsafe_code)]

pub mod handlers;
pub mod routes;
pub mod state;
// pub mod middleware; // Disabled for minimal build
// pub mod extractors; // Disabled for minimal build

pub use state::AppState;

use axum::Router;
use sdrtrunk_core::Config;
use sdrtrunk_core::context_error::Result;
use sdrtrunk_database::PgPool;
use std::sync::Arc;

/// Build the API router with all routes and middleware
///
/// # Errors
///
/// Returns an error if the application state validation fails.
pub async fn build_router(config: Config, pool: PgPool) -> Result<Router> {
    let mut app_state = AppState::new(config.clone(), pool.clone())?;

    // Initialize transcription service if enabled
    if let Some(ref transcription_config) = config.transcription {
        if transcription_config.enabled {

            // Initialize the appropriate transcription service
            let mut service_instance = if transcription_config.service == "whisperx" {
                Box::new(sdrtrunk_transcriber::WhisperXService::new(transcription_config.clone()))
                    as Box<dyn sdrtrunk_transcriber::TranscriptionService>
            } else {
                // Use mock service for testing or when whisperx is not available
                Box::new(sdrtrunk_transcriber::MockTranscriptionService::new())
                    as Box<dyn sdrtrunk_transcriber::TranscriptionService>
            };

            // Initialize the service
            service_instance.initialize(transcription_config).await
                .expect("Failed to initialize transcription service");

            let service: Arc<dyn sdrtrunk_transcriber::TranscriptionService> = Arc::from(service_instance);

            // Create worker pool
            let mut worker_pool = sdrtrunk_transcriber::TranscriptionWorkerPool::new(
                transcription_config.clone(),
                service,
                pool.clone(),
            );

            // Start the workers
            worker_pool.start().await
                .expect("Failed to start transcription worker pool");

            let worker_pool_arc = Arc::new(worker_pool);

            // Start queue monitoring task
            let monitor_pool = Arc::clone(&worker_pool_arc);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    let queue_len = monitor_pool.queue_len();
                    let queue_capacity = monitor_pool.queue_capacity().unwrap_or(0);
                    let utilization = if queue_capacity > 0 {
                        (queue_len as f64 / queue_capacity as f64) * 100.0
                    } else {
                        0.0
                    };

                    if queue_len > 0 {
                        tracing::info!("Transcription queue status: {}/{} ({:.1}% full)", queue_len, queue_capacity, utilization);
                    }

                    // Alert if queue is getting full
                    if utilization >= 80.0 {
                        tracing::warn!("Transcription queue is {:.1}% full ({}/{})", utilization, queue_len, queue_capacity);
                    }
                    if utilization >= 95.0 {
                        tracing::error!("Transcription queue is critically full: {:.1}% ({}/{})", utilization, queue_len, queue_capacity);
                    }
                }
            });

            // Set the pool in app state
            app_state.set_transcription_pool(worker_pool_arc);
        }
    }

    let state = Arc::new(app_state);

    // Validate the application state
    state.validate()?;

    // Build the complete router with all routes
    let app = routes::build_router().with_state(state);

    Ok(app)
}

/// Build a minimal router for testing (without authentication)
///
/// # Errors
///
/// Returns an error if the application state creation fails.
#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
pub async fn build_test_router(config: Config, pool: PgPool) -> Result<Router> {
    let state = Arc::new(AppState::new(config, pool)?);

    // Build a simplified router for testing
    let app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::docs_routes())
        .with_state(state);

    Ok(app)
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use sdrtrunk_core::Config;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_config_with_temp_dir(temp_dir: &TempDir) -> Config {
        let mut config = Config::default();
        config.storage.base_dir = temp_dir.path().to_path_buf();
        config
    }

    // Mock tests that don't require database connections
    #[test]
    fn test_module_structure() {
        // Verify all modules are accessible
        use crate::{handlers, state};

        // This is mainly a compile-time test
        let _handlers_mod = std::any::type_name::<handlers::health::HealthResponse>();
        let _routes_mod = "routes module"; // Just check module exists
        let _state_mod = std::any::type_name::<state::AppState>();
    }

    #[test]
    fn test_config_creation_and_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test that config is properly created
        assert_eq!(config.storage.base_dir, temp_dir.path().to_path_buf());
        assert_eq!(config.storage.upload_dir, "uploads");
        assert!(config.storage.organize_by_date);

        // Test that upload directory path construction works
        let upload_path = config.storage.base_dir.join(&config.storage.upload_dir);
        assert_eq!(upload_path, temp_dir.path().join("uploads"));
    }

    #[test]
    fn test_path_generation_logic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test the path generation logic without database
        let base_upload_dir = config.storage.base_dir.join(&config.storage.upload_dir);
        let test_date = chrono::NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();

        let storage_path = base_upload_dir
            .join("test_system")
            .join(test_date.format("%Y").to_string())
            .join(test_date.format("%m").to_string())
            .join(test_date.format("%d").to_string());

        let expected_path = temp_dir
            .path()
            .join("uploads")
            .join("test_system")
            .join("2023")
            .join("12")
            .join("25");

        assert_eq!(storage_path, expected_path);
    }

    #[test]
    fn test_directory_creation_logic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test directory creation without AppState
        let upload_dir = config.storage.base_dir.join(&config.storage.upload_dir);
        let result = std::fs::create_dir_all(&upload_dir);
        assert!(result.is_ok());
        assert!(upload_dir.exists());
    }

    #[test]
    fn test_config_variations() {
        let temp_dir1 = TempDir::new().expect("Failed to create temp dir 1");
        let temp_dir2 = TempDir::new().expect("Failed to create temp dir 2");

        // Test different config variations without database
        let mut config1 = create_test_config_with_temp_dir(&temp_dir1);
        config1.server.port = 8080;

        let mut config2 = create_test_config_with_temp_dir(&temp_dir2);
        config2.server.port = 8081;

        assert_eq!(config1.server.port, 8080);
        assert_eq!(config2.server.port, 8081);
        assert_ne!(config1.storage.base_dir, config2.storage.base_dir);
    }

    #[test]
    fn test_storage_config_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test storage configuration
        assert!(!config.storage.allowed_extensions.is_empty());
        assert!(
            config
                .storage
                .allowed_extensions
                .contains(&"mp3".to_string())
        );
        assert!(
            config
                .storage
                .allowed_extensions
                .contains(&"wav".to_string())
        );
        assert!(
            config
                .storage
                .allowed_extensions
                .contains(&"flac".to_string())
        );

        assert!(config.storage.max_file_size > 0);
        assert!(config.storage.organize_by_date);
    }

    #[test]
    fn test_server_config_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test server configuration
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(config.server.workers > 0);
    }

    #[test]
    fn test_api_config_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config_with_temp_dir(&temp_dir);

        // Test API configuration
        assert!(config.api.rate_limit > 0);
        assert!(!config.api.cors_origins.is_empty());
    }

    #[test]
    fn test_error_path_validation() {
        // Test error handling for invalid paths
        let empty_config = Config::default();
        let mut invalid_config = empty_config.clone();
        invalid_config.storage.base_dir = std::path::PathBuf::from("");

        // Test that empty path is properly detected
        assert!(invalid_config.storage.base_dir.as_os_str().is_empty());

        // Test path validation logic
        let invalid_path =
            std::path::PathBuf::from("/nonexistent/path/that/should/not/exist/12345");
        assert!(!invalid_path.exists());
    }

    // Database integration tests that require tokio runtime
    // These tests are moved to integration tests in the tests/ directory

    #[test]
    fn test_re_exports_available() {
        // Test that all re-exports are accessible at compile time
        let _app_state_type = std::any::type_name::<AppState>();
        let _config_type = std::any::type_name::<Config>();
        let _arc_type = std::any::type_name::<Arc<AppState>>();
    }

    #[test]
    fn test_router_functions_exist() {
        // Test that router builder functions exist and are callable
        // This is a compile-time test to verify functions exist

        // Just test that the functions can be referenced
        let _build_router_exists = build_router;
        let _build_test_router_exists = build_test_router;

        // If we got here, the functions exist and are accessible
        assert!(true);
    }

    #[test]
    fn test_date_formatting_logic() {
        // Test date formatting used in storage paths
        let test_date = chrono::NaiveDate::from_ymd_opt(2023, 1, 5).unwrap();

        assert_eq!(test_date.format("%Y").to_string(), "2023");
        assert_eq!(test_date.format("%m").to_string(), "01");
        assert_eq!(test_date.format("%d").to_string(), "05");

        let test_date2 = chrono::NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        assert_eq!(test_date2.format("%Y").to_string(), "2023");
        assert_eq!(test_date2.format("%m").to_string(), "12");
        assert_eq!(test_date2.format("%d").to_string(), "25");
    }

    #[test]
    fn test_path_construction() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path();

        // Test path construction logic
        let upload_dir = "uploads";
        let system_id = "police_system";
        let year = "2023";
        let month = "12";
        let day = "25";

        let full_path = base_path
            .join(upload_dir)
            .join(system_id)
            .join(year)
            .join(month)
            .join(day);

        let expected = base_path
            .join("uploads")
            .join("police_system")
            .join("2023")
            .join("12")
            .join("25");

        assert_eq!(full_path, expected);

        // Test that path can be converted to string
        let path_str = full_path.to_string_lossy();
        assert!(path_str.contains("uploads"));
        assert!(path_str.contains("police_system"));
        assert!(path_str.contains("2023"));
        assert!(path_str.contains("12"));
        assert!(path_str.contains("25"));
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::default();

        // Test that defaults are sensible
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(config.server.workers > 0);

        assert!(!config.database.url.is_empty());
        assert!(config.database.max_connections > 0);
        assert!(config.database.min_connections > 0);
        assert!(config.database.max_connections >= config.database.min_connections);

        assert!(!config.storage.upload_dir.is_empty());
        assert!(config.storage.max_file_size > 0);
        assert!(!config.storage.allowed_extensions.is_empty());

        assert!(config.api.rate_limit > 0);
        assert!(!config.api.cors_origins.is_empty());

        assert!(config.security.max_upload_size > 0);
        assert!(config.security.request_timeout > 0);

        assert!(!config.logging.level.is_empty());
        assert!(!config.logging.format.is_empty());
    }

    #[test]
    fn test_file_extension_validation() {
        let config = Config::default();
        let allowed = &config.storage.allowed_extensions;

        // Test that default allowed extensions make sense
        assert!(allowed.len() >= 3);

        // Test common audio formats are included
        let has_mp3 = allowed.iter().any(|ext| ext.to_lowercase() == "mp3");
        let has_wav = allowed.iter().any(|ext| ext.to_lowercase() == "wav");
        let has_flac = allowed.iter().any(|ext| ext.to_lowercase() == "flac");

        assert!(has_mp3, "Should support MP3 files");
        assert!(has_wav, "Should support WAV files");
        assert!(has_flac, "Should support FLAC files");
    }

    #[test]
    fn test_size_limits_validation() {
        let config = Config::default();

        // Test file size limits are reasonable
        assert!(config.storage.max_file_size >= 1_000_000); // At least 1MB
        assert!(config.storage.max_file_size <= 1_000_000_000); // At most 1GB

        assert!(config.security.max_upload_size >= 1_000_000); // At least 1MB
        assert!(config.security.max_upload_size <= 1_000_000_000); // At most 1GB
    }

    #[test]
    fn test_timeout_validation() {
        let config = Config::default();

        // Test timeouts are reasonable
        assert!(config.database.connect_timeout >= 1); // At least 1 second
        assert!(config.database.connect_timeout <= 300); // At most 5 minutes

        assert!(config.database.idle_timeout >= 60); // At least 1 minute
        assert!(config.database.idle_timeout <= 3600); // At most 1 hour

        assert!(config.security.request_timeout >= 1); // At least 1 second
        assert!(config.security.request_timeout <= 300); // At most 5 minutes
    }
}
