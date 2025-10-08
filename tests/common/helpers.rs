//! Test helper functions and utilities

use sdrtrunk_core::{context_error::Result, context_error};
use reqwest::multipart::{Form, Part};
use sdrtrunk_core::Config;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Once;
use tokio::net::TcpListener;

static INIT_LOGGER: Once = Once::new();

/// Initialize test logging (call once per test process)
pub fn init_test_logging() {
    INIT_LOGGER.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .try_init();
    });
}

/// Find an available port for testing
pub async fn find_available_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// Create a test HTTP client
pub fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

/// Create a multipart form for file upload testing
pub async fn create_upload_form(file_path: &Path, metadata: HashMap<String, String>) -> Result<Form> {
    let file_content = tokio::fs::read(file_path).await?;
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("test.mp3");

    let mut form = Form::new()
        .part("file", Part::bytes(file_content).file_name(filename.to_string()));

    // Add metadata fields
    for (key, value) in metadata {
        form = form.text(key, value);
    }

    Ok(form)
}

/// Create test metadata for uploads
pub fn create_upload_metadata(system_id: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("system_id".to_string(), system_id.to_string());
    metadata.insert("system_label".to_string(), "Test System".to_string());
    metadata.insert("talkgroup_id".to_string(), "12345".to_string());
    metadata.insert("talkgroup_label".to_string(), "Test Talkgroup".to_string());
    metadata.insert("source_radio_id".to_string(), "678901".to_string());
    metadata.insert("frequency".to_string(), "854000000".to_string());
    metadata
}

/// Wait for server to be ready
pub async fn wait_for_server(base_url: &str, timeout_secs: u64) -> Result<()> {
    let client = create_test_client();
    let health_url = format!("{base_url}/health");
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        if let Ok(response) = client.get(&health_url).send().await {
            if response.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Err(context_error!("Server did not become ready within {timeout_secs} seconds"))
}

/// Create test directories structure
pub fn create_test_dirs(base: &Path) -> Result<TestDirectories> {
    let uploads = base.join("uploads");
    let audio = base.join("audio");
    let temp = base.join("temp");

    std::fs::create_dir_all(&uploads)?;
    std::fs::create_dir_all(&audio)?;
    std::fs::create_dir_all(&temp)?;

    Ok(TestDirectories {
        base: base.to_path_buf(),
        uploads,
        audio,
        temp,
    })
}

/// Test directories structure
#[derive(Debug)]
pub struct TestDirectories {
    /// Base directory
    pub base: PathBuf,
    /// Uploads directory
    pub uploads: PathBuf,
    /// Audio files directory
    pub audio: PathBuf,
    /// Temporary files directory
    pub temp: PathBuf,
}

impl TestDirectories {
    /// Clean all test directories
    pub fn clean(&self) -> Result<()> {
        if self.base.exists() {
            std::fs::remove_dir_all(&self.base)?;
        }
        Ok(())
    }
}

/// HTTP test client wrapper with common functionality
pub struct TestHttpClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl TestHttpClient {
    /// Create a new test HTTP client
    pub fn new(base_url: String) -> Self {
        Self {
            client: create_test_client(),
            base_url,
            api_key: None,
        }
    }

    /// Set API key for authenticated requests
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Get request with optional authentication
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.get(&url);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("X-API-Key", api_key);
        }
        
        Ok(request.send().await?)
    }

    /// Post JSON request
    pub async fn post_json<T: serde::Serialize>(&self, path: &str, data: &T) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.post(&url).json(data);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("X-API-Key", api_key);
        }
        
        Ok(request.send().await?)
    }

    /// Post multipart form request
    pub async fn post_form(&self, path: &str, form: Form) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.post(&url).multipart(form);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("X-API-Key", api_key);
        }
        
        Ok(request.send().await?)
    }

    /// Upload a file with metadata
    pub async fn upload_file(&self, file_path: &Path, metadata: HashMap<String, String>) -> Result<reqwest::Response> {
        let form = create_upload_form(file_path, metadata).await?;
        self.post_form("/api/v1/upload", form).await
    }

    /// Get health status
    pub async fn health(&self) -> Result<reqwest::Response> {
        self.get("/health").await
    }

    /// Get system statistics
    pub async fn system_stats(&self, system_id: &str) -> Result<reqwest::Response> {
        let path = format!("/api/v1/systems/{system_id}/stats");
        self.get(&path).await
    }

    /// Get radio calls with pagination
    pub async fn get_calls(&self, page: u32, per_page: u32) -> Result<reqwest::Response> {
        let path = format!("/api/v1/calls?page={page}&per_page={per_page}");
        self.get(&path).await
    }

    /// Get radio calls for a specific system
    pub async fn get_system_calls(&self, system_id: &str, page: u32, per_page: u32) -> Result<reqwest::Response> {
        let path = format!("/api/v1/systems/{system_id}/calls?page={page}&per_page={per_page}");
        self.get(&path).await
    }
}

/// Assert that a JSON response matches expected structure
pub async fn assert_json_response<T: serde::de::DeserializeOwned + std::fmt::Debug>(
    response: reqwest::Response,
    expected_status: reqwest::StatusCode,
) -> Result<T> {
    assert_eq!(response.status(), expected_status, "Unexpected HTTP status");
    
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");
    
    assert!(
        content_type.contains("application/json"),
        "Expected JSON content-type, got: {content_type}"
    );
    
    let text = response.text().await?;
    let parsed: T = serde_json::from_str(&text)
        .map_err(|e| context_error!("Failed to parse JSON response: {e}\nResponse: {text}"))?;
    
    Ok(parsed)
}

/// Assert that an error response has the expected error code
pub async fn assert_error_response(
    response: reqwest::Response,
    expected_status: reqwest::StatusCode,
    expected_error_code: &str,
) -> Result<sdrtrunk_core::types::ErrorResponse> {
    let error_response = assert_json_response(response, expected_status).await?;
    assert_eq!(error_response.code, expected_error_code);
    assert!(!error_response.success);
    Ok(error_response)
}

/// Create test audio file with specific duration and format
pub async fn create_test_audio_file(
    dir: &Path,
    filename: &str,
    duration_ms: u64,
    format: &str,
) -> Result<PathBuf> {
    let file_path = dir.join(filename);
    
    // Create minimal audio file content based on format
    let content = match format.to_lowercase().as_str() {
        "mp3" => create_minimal_mp3(duration_ms),
        "wav" => create_minimal_wav(duration_ms),
        "flac" => create_minimal_flac(duration_ms),
        _ => return Err(context_error!("Unsupported audio format: {format}")),
    };
    
    tokio::fs::write(&file_path, content).await?;
    Ok(file_path)
}

/// Create minimal MP3 file content
fn create_minimal_mp3(duration_ms: u64) -> Vec<u8> {
    let mut content = Vec::new();
    
    // ID3v2 header
    content.extend_from_slice(b"ID3\x03\x00\x00\x00\x00\x00\x23");
    
    // Minimal MP3 frame (calculate frames needed for duration)
    let frames_needed = duration_ms / 26; // Approximately 26ms per frame at 44.1kHz
    let mp3_frame = vec![
        0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    
    for _ in 0..frames_needed {
        content.extend_from_slice(&mp3_frame);
    }
    
    content
}

/// Create minimal WAV file content
fn create_minimal_wav(duration_ms: u64) -> Vec<u8> {
    let sample_rate = 44100u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * bits_per_sample / 8;
    
    let data_size = (sample_rate as u64 * duration_ms * u64::from(channels) * u64::from(bits_per_sample) / 8 / 1000) as u32;
    let chunk_size = 36 + data_size;
    
    let mut content = Vec::new();
    
    // RIFF header
    content.extend_from_slice(b"RIFF");
    content.extend_from_slice(&chunk_size.to_le_bytes());
    content.extend_from_slice(b"WAVE");
    
    // fmt subchunk
    content.extend_from_slice(b"fmt ");
    content.extend_from_slice(&16u32.to_le_bytes()); // Subchunk size
    content.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
    content.extend_from_slice(&channels.to_le_bytes());
    content.extend_from_slice(&sample_rate.to_le_bytes());
    content.extend_from_slice(&byte_rate.to_le_bytes());
    content.extend_from_slice(&block_align.to_le_bytes());
    content.extend_from_slice(&bits_per_sample.to_le_bytes());
    
    // data subchunk
    content.extend_from_slice(b"data");
    content.extend_from_slice(&data_size.to_le_bytes());
    
    // Add silence data
    content.resize(content.len() + data_size as usize, 0);
    
    content
}

/// Create minimal FLAC file content
fn create_minimal_flac(duration_ms: u64) -> Vec<u8> {
    // This is a very simplified FLAC file that may not be fully valid
    // but should be sufficient for basic testing
    let mut content = Vec::new();
    
    // FLAC signature
    content.extend_from_slice(b"fLaC");
    
    // STREAMINFO metadata block
    content.extend_from_slice(&[
        0x00, // Last block flag + block type
        0x00, 0x00, 0x22, // Block length (34 bytes)
    ]);
    
    // STREAMINFO content (simplified)
    content.extend_from_slice(&[0x10, 0x00]); // min block size
    content.extend_from_slice(&[0x10, 0x00]); // max block size
    content.extend_from_slice(&[0x00, 0x00, 0x00]); // min frame size
    content.extend_from_slice(&[0x00, 0x00, 0x00]); // max frame size
    content.extend_from_slice(&[0x0A, 0xC4, 0x42]); // sample rate (44100) + channels + bits per sample
    
    let total_samples = 44100u64 * duration_ms / 1000;
    content.extend_from_slice(&total_samples.to_be_bytes()[3..8]); // total samples (36 bits)
    content.extend_from_slice(&[0u8; 16]); // MD5 signature (zeros)
    
    // Add minimal frame data
    content.extend_from_slice(&[0xFF, 0xF8, 0x69, 0x04, 0x00, 0x00]);
    
    content
}

/// Performance testing utilities
pub struct PerformanceTimer {
    start: tokio::time::Instant,
    name: String,
}

impl PerformanceTimer {
    /// Start a new performance timer
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            start: tokio::time::Instant::now(),
            name: name.into(),
        }
    }

    /// Stop the timer and return elapsed time
    pub fn stop(self) -> tokio::time::Duration {
        let elapsed = self.start.elapsed();
        tracing::info!("Performance timer '{}': {:?}", self.name, elapsed);
        elapsed
    }

    /// Check elapsed time without stopping
    pub fn elapsed(&self) -> tokio::time::Duration {
        self.start.elapsed()
    }
}

/// Assert that an operation completes within a time limit
pub async fn assert_timeout<F, Fut, T>(
    future: F,
    timeout: tokio::time::Duration,
    operation_name: &str,
) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(timeout, future()).await {
        Ok(result) => result,
        Err(_) => Err(context_error!("Operation '{operation_name}' timed out after {timeout:?}")),
    }
}

/// Performance assertion helper for enforcing response time targets
pub struct PerformanceAssertion {
    operation: String,
    target_ms: u64,
    max_ms: u64,
}

impl PerformanceAssertion {
    /// Create a new performance assertion
    ///
    /// # Arguments
    ///
    /// * `operation` - Name of the operation being measured
    /// * `target_ms` - Target response time in milliseconds (warning if exceeded)
    /// * `max_ms` - Maximum acceptable response time in milliseconds (panic if exceeded)
    pub fn new(operation: impl Into<String>, target_ms: u64, max_ms: u64) -> Self {
        Self {
            operation: operation.into(),
            target_ms,
            max_ms,
        }
    }

    /// Execute a future and assert it completes within performance targets
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails or exceeds max time limit.
    pub async fn assert<F, Fut, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let start = tokio::time::Instant::now();
        let result = f().await;
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        if elapsed_ms > self.max_ms {
            panic!(
                "Performance assertion FAILED: '{}' took {}ms (max: {}ms)",
                self.operation, elapsed_ms, self.max_ms
            );
        }

        if elapsed_ms > self.target_ms {
            tracing::warn!(
                "Performance target EXCEEDED: '{}' took {}ms (target: {}ms)",
                self.operation,
                elapsed_ms,
                self.target_ms
            );
        } else {
            tracing::debug!(
                "Performance OK: '{}' took {}ms (target: {}ms, max: {}ms)",
                self.operation,
                elapsed_ms,
                self.target_ms,
                self.max_ms
            );
        }

        result
    }

    /// Execute a synchronous operation and assert it completes within performance targets
    pub fn assert_sync<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        if elapsed_ms > self.max_ms {
            panic!(
                "Performance assertion FAILED: '{}' took {}ms (max: {}ms)",
                self.operation, elapsed_ms, self.max_ms
            );
        }

        if elapsed_ms > self.target_ms {
            tracing::warn!(
                "Performance target EXCEEDED: '{}' took {}ms (target: {}ms)",
                self.operation,
                elapsed_ms,
                self.target_ms
            );
        } else {
            tracing::debug!(
                "Performance OK: '{}' took {}ms (target: {}ms, max: {}ms)",
                self.operation,
                elapsed_ms,
                self.target_ms,
                self.max_ms
            );
        }

        result
    }
}

/// Memory usage monitoring for performance tests
pub struct MemoryMonitor {
    initial: Option<usize>,
}

impl MemoryMonitor {
    /// Start monitoring memory usage
    pub fn start() -> Self {
        Self {
            initial: Self::get_memory_usage(),
        }
    }

    /// Get current memory usage increase
    pub fn usage_increase(&self) -> Option<usize> {
        if let (Some(initial), Some(current)) = (self.initial, Self::get_memory_usage()) {
            Some(current.saturating_sub(initial))
        } else {
            None
        }
    }

    /// Get current memory usage (platform-dependent)
    fn get_memory_usage() -> Option<usize> {
        // This is a simplified implementation
        // In a real scenario, you might use platform-specific APIs
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
                for line in contents.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(value) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = value.parse::<usize>() {
                                return Some(kb * 1024); // Convert KB to bytes
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_find_available_port() {
        let port = find_available_port().await.unwrap();
        assert!(port > 0);
        assert!(port < 65536);
    }

    #[test]
    fn test_create_test_client() {
        let client = create_test_client();
        // Just verify it was created successfully
        assert!(client.get("http://example.com").build().is_ok());
    }

    #[test]
    fn test_create_upload_metadata() {
        let metadata = create_upload_metadata("test_system");
        assert_eq!(metadata.get("system_id"), Some(&"test_system".to_string()));
        assert_eq!(metadata.get("talkgroup_id"), Some(&"12345".to_string()));
        assert!(metadata.contains_key("frequency"));
    }

    #[test]
    fn test_create_test_dirs() {
        let temp_dir = tempdir().unwrap();
        let test_dirs = create_test_dirs(temp_dir.path()).unwrap();
        
        assert!(test_dirs.uploads.exists());
        assert!(test_dirs.audio.exists());
        assert!(test_dirs.temp.exists());
        
        test_dirs.clean().unwrap();
    }

    #[test]
    fn test_test_http_client_creation() {
        let client = TestHttpClient::new("http://localhost:8080".to_string());
        let client_with_key = client.with_api_key("test_key".to_string());
        
        assert!(client_with_key.api_key.is_some());
        assert_eq!(client_with_key.api_key.unwrap(), "test_key");
    }

    #[tokio::test]
    async fn test_create_test_audio_files() {
        let temp_dir = tempdir().unwrap();
        
        let mp3_path = create_test_audio_file(temp_dir.path(), "test.mp3", 1000, "mp3").await.unwrap();
        assert!(mp3_path.exists());
        assert_eq!(mp3_path.extension().unwrap(), "mp3");
        
        let wav_path = create_test_audio_file(temp_dir.path(), "test.wav", 2000, "wav").await.unwrap();
        assert!(wav_path.exists());
        assert_eq!(wav_path.extension().unwrap(), "wav");
    }

    #[test]
    fn test_performance_timer() {
        let timer = PerformanceTimer::start("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.stop();
        
        assert!(elapsed >= std::time::Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_assert_timeout_success() {
        let result = assert_timeout(
            || async { Ok::<i32, sdrtrunk_core::context_error::ContextError>(42) },
            tokio::time::Duration::from_millis(100),
            "test",
        ).await.unwrap();
        
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_assert_timeout_failure() {
        let result = assert_timeout(
            || async {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                Ok::<i32, sdrtrunk_core::context_error::ContextError>(42)
            },
            tokio::time::Duration::from_millis(50),
            "test",
        ).await;
        
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_monitor() {
        let monitor = MemoryMonitor::start();
        
        // Allocate some memory
        let _data: Vec<u8> = vec![0; 1024 * 1024]; // 1MB
        
        // Memory usage might or might not be detectable depending on the platform
        let usage = monitor.usage_increase();
        // We can't assert specific values due to platform differences
        // but we can verify the function doesn't panic
        println!("Memory usage increase: {usage:?}");
    }

    #[test]
    fn test_minimal_audio_formats() {
        let mp3_data = create_minimal_mp3(1000);
        assert!(!mp3_data.is_empty());
        assert!(mp3_data.starts_with(b"ID3"));

        let wav_data = create_minimal_wav(1000);
        assert!(!wav_data.is_empty());
        assert!(wav_data.starts_with(b"RIFF"));

        let flac_data = create_minimal_flac(1000);
        assert!(!flac_data.is_empty());
        assert!(flac_data.starts_with(b"fLaC"));
    }

    #[tokio::test]
    async fn test_performance_assertion_success() {
        let perf = PerformanceAssertion::new("fast operation", 50, 100);

        let result = perf
            .assert(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok::<i32, sdrtrunk_core::context_error::ContextError>(42)
            })
            .await
            .unwrap();

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_performance_assertion_exceeds_target() {
        let perf = PerformanceAssertion::new("slow operation", 10, 100);

        // This should succeed but log a warning
        let result = perf
            .assert(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                Ok::<i32, sdrtrunk_core::context_error::ContextError>(42)
            })
            .await
            .unwrap();

        assert_eq!(result, 42);
    }

    #[tokio::test]
    #[should_panic(expected = "Performance assertion FAILED")]
    async fn test_performance_assertion_exceeds_max() {
        let perf = PerformanceAssertion::new("too slow operation", 10, 50);

        let _result = perf
            .assert(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                Ok::<i32, sdrtrunk_core::context_error::ContextError>(42)
            })
            .await
            .unwrap();
    }

    #[test]
    fn test_performance_assertion_sync_success() {
        let perf = PerformanceAssertion::new("fast sync operation", 50, 100);

        let result = perf.assert_sync(|| {
            std::thread::sleep(std::time::Duration::from_millis(5));
            42
        });

        assert_eq!(result, 42);
    }

    #[test]
    #[should_panic(expected = "Performance assertion FAILED")]
    fn test_performance_assertion_sync_exceeds_max() {
        let perf = PerformanceAssertion::new("too slow sync operation", 10, 50);

        let _result = perf.assert_sync(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            42
        });
    }
}