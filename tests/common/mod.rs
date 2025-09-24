//! Common test utilities and fixtures for integration tests

use sdrtrunk_core::{context_error::Result, context_error};
use sdrtrunk_core::{
    types::{ApiKey, FileData, RadioCall, TranscriptionStatus},
    Config,
};
use sdrtrunk_database::{models::RadioCallDb, Database};
use std::path::PathBuf;
use tempfile::TempDir;
use testcontainers::{clients, runners::AsyncRunner, ContainerAsync, Image};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

pub mod fixtures;
pub mod helpers;

pub use fixtures::*;
pub use helpers::*;

/// Test database container wrapper
pub struct TestDatabase {
    pub container: ContainerAsync<Postgres>,
    pub database: Database,
    pub connection_string: String,
}

impl TestDatabase {
    /// Create a new test database with PostgreSQL container
    pub async fn new() -> Result<Self> {
        let docker = clients::Cli::default();
        let postgres = Postgres::default().with_tag("16-alpine");

        let container = postgres.start(&docker).await?;
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let connection_string = format!("postgresql://postgres:postgres@{host}:{port}/postgres");

        let mut config = Config::default();
        config.database.url = connection_string.clone();
        config.database.max_connections = 5;
        config.database.min_connections = 1;

        let database = Database::new(&config).await?;
        
        // Run migrations
        database.migrate().await?;

        Ok(Self {
            container,
            database,
            connection_string,
        })
    }

    /// Get the database instance
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get the connection string
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

/// Test configuration builder
pub struct TestConfigBuilder {
    config: Config,
    temp_dir: Option<TempDir>,
}

impl TestConfigBuilder {
    /// Create a new test configuration builder
    pub fn new() -> Self {
        let mut config = Config::default();
        config.database.url = "postgresql://test:test@localhost/test".to_string();
        
        Self {
            config,
            temp_dir: None,
        }
    }

    /// Set database URL
    pub fn with_database_url(mut self, url: String) -> Self {
        self.config.database.url = url;
        self
    }

    /// Create temporary storage directory
    pub fn with_temp_storage(mut self) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        self.config.storage.base_dir = temp_dir.path().to_path_buf();
        self.temp_dir = Some(temp_dir);
        Ok(self)
    }

    /// Set server port
    pub fn with_port(mut self, port: u16) -> Self {
        self.config.server.port = port;
        self
    }

    /// Disable authentication for testing
    pub fn without_auth(mut self) -> Self {
        self.config.api.enable_auth = false;
        self.config.security.require_api_key = false;
        self
    }

    /// Build the configuration
    pub fn build(self) -> (Config, Option<TempDir>) {
        (self.config, self.temp_dir)
    }
}

impl Default for TestConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a test radio call with minimal required fields
pub fn create_test_radio_call(system_id: &str) -> RadioCall {
    let mut call = RadioCall::default();
    call.id = Some(Uuid::new_v4());
    call.system_id = system_id.to_string();
    call.system_label = Some("Test System".to_string());
    call.talkgroup_id = Some(12345);
    call.talkgroup_label = Some("Test Talkgroup".to_string());
    call.source_radio_id = Some(678901);
    call.frequency = Some(854_000_000);
    call.audio_filename = Some("test_audio.mp3".to_string());
    call.audio_file_path = Some("/test/path/test_audio.mp3".to_string());
    call.duration_seconds = Some(30.5);
    call.transcription_status = TranscriptionStatus::None;
    call
}

/// Create a test API key
pub fn create_test_api_key(id: &str, name: &str) -> ApiKey {
    ApiKey {
        id: id.to_string(),
        key_hash: "hashed_test_key".to_string(),
        name: name.to_string(),
        created_at: chrono::Utc::now(),
        expires_at: None,
        allowed_ips: vec!["127.0.0.1".to_string()],
        allowed_systems: vec!["test_system".to_string()],
        active: true,
    }
}

/// Create test file data from filename
pub fn create_test_file_data(filename: &str) -> FileData {
    FileData {
        date: "20240315".to_string(),
        time: "142530".to_string(),
        unixtime: 1710509130,
        talkgroup_id: 52197,
        talkgroup_name: "TG52197".to_string(),
        radio_id: 1234567,
        duration: "00:30".to_string(),
        filename: filename.to_string(),
        filepath: format!("/test/path/{filename}"),
    }
}

/// Wait for a condition to be true with timeout
pub async fn wait_for_condition<F, Fut>(mut condition: F, timeout_ms: u64) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    Err(context_error!("Condition not met within timeout"))
}

/// Create a minimal MP3 file for testing
pub fn create_test_mp3_file(dir: &std::path::Path, filename: &str) -> Result<PathBuf> {
    let file_path = dir.join(filename);
    
    // Minimal MP3 header (ID3v2 + basic frame)
    let mp3_data = vec![
        // ID3v2 header
        0x49, 0x44, 0x33, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23,
        // Basic MP3 frame
        0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    
    std::fs::write(&file_path, mp3_data)?;
    Ok(file_path)
}

/// Assert that two timestamps are approximately equal (within 1 second)
pub fn assert_timestamp_eq(actual: chrono::DateTime<chrono::Utc>, expected: chrono::DateTime<chrono::Utc>) {
    let diff = (actual - expected).num_seconds().abs();
    assert!(diff <= 1, "Timestamps differ by more than 1 second: actual={actual}, expected={expected}");
}

/// Generate random test data
pub fn generate_random_system_id() -> String {
    format!("test_system_{}", uuid::Uuid::new_v4().simple())
}

/// Generate random test filename in SDRTrunk format
pub fn generate_random_sdrtrunk_filename() -> String {
    let now = chrono::Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let time = now.format("%H%M%S").to_string();
    // Use UUID bits for pseudo-random numbers
    let uuid = uuid::Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let talkgroup = ((bytes[0] as u16) << 8 | bytes[1] as u16) % 99999 + 1;
    let radio_id = ((bytes[2] as u32) << 24 | (bytes[3] as u32) << 16 |
                   (bytes[4] as u32) << 8 | bytes[5] as u32) % 9999999 + 1000000;
    
    format!("{date}_{time}_TestSystem_TG{talkgroup}_FROM_{radio_id}.mp3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_radio_call() {
        let call = create_test_radio_call("test_system");
        assert_eq!(call.system_id, "test_system");
        assert!(call.id.is_some());
        assert_eq!(call.system_label, Some("Test System".to_string()));
        assert_eq!(call.talkgroup_id, Some(12345));
    }

    #[test]
    fn test_create_test_api_key() {
        let key = create_test_api_key("test_key_id", "Test Key");
        assert_eq!(key.id, "test_key_id");
        assert_eq!(key.name, "Test Key");
        assert!(key.active);
        assert!(!key.allowed_ips.is_empty());
    }

    #[test]
    fn test_create_test_file_data() {
        let file_data = create_test_file_data("test.mp3");
        assert_eq!(file_data.filename, "test.mp3");
        assert_eq!(file_data.talkgroup_id, 52197);
        assert_eq!(file_data.radio_id, 1234567);
    }

    #[test]
    fn test_generate_random_system_id() {
        let id1 = generate_random_system_id();
        let id2 = generate_random_system_id();
        
        assert!(id1.starts_with("test_system_"));
        assert!(id2.starts_with("test_system_"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_random_sdrtrunk_filename() {
        let filename = generate_random_sdrtrunk_filename();
        
        assert!(filename.ends_with(".mp3"));
        assert!(filename.contains("TestSystem"));
        assert!(filename.contains("TG"));
        assert!(filename.contains("FROM_"));
        
        // Should be parseable by the SDRTrunk filename parser
        assert!(filename.split('_').count() >= 5);
    }

    #[test]
    fn test_test_config_builder() {
        let builder = TestConfigBuilder::new();
        let (config, _temp_dir) = builder
            .with_port(9090)
            .without_auth()
            .build();
            
        assert_eq!(config.server.port, 9090);
        assert!(!config.api.enable_auth);
        assert!(!config.security.require_api_key);
    }

    #[tokio::test]
    async fn test_create_test_mp3_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = create_test_mp3_file(temp_dir.path(), "test.mp3").unwrap();
        
        assert!(file_path.exists());
        assert!(file_path.extension().unwrap() == "mp3");
        
        let metadata = std::fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_assert_timestamp_eq() {
        let now = chrono::Utc::now();
        let close = now + chrono::Duration::milliseconds(500);
        
        // Should not panic
        assert_timestamp_eq(now, close);
        
        // Test that it would panic for times too far apart
        let far = now + chrono::Duration::seconds(2);
        let result = std::panic::catch_unwind(|| assert_timestamp_eq(now, far));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wait_for_condition_success() {
        let mut counter = 0;
        
        let result = wait_for_condition(
            || {
                counter += 1;
                async move { counter >= 3 }
            },
            1000,
        ).await;
        
        assert!(result.is_ok());
        assert!(counter >= 3);
    }

    #[tokio::test]
    async fn test_wait_for_condition_timeout() {
        let result = wait_for_condition(
            || async { false },
            100,
        ).await;
        
        assert!(result.is_err());
    }
}