//! Configuration management for `SDRTrunk` transcriber

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// File storage configuration
    pub storage: StorageConfig,

    /// API configuration
    pub api: ApiConfig,

    /// Security configuration
    pub security: SecurityConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Monitor configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<MonitorConfig>,

    /// Transcription configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcription: Option<TranscriptionConfig>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,

    /// Number of worker threads
    #[serde(default = "default_workers")]
    pub workers: usize,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,

    /// Maximum number of connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of connections
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,

    /// Idle timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base directory for file storage
    pub base_dir: PathBuf,

    /// Upload directory (relative to `base_dir`)
    #[serde(default = "default_upload_dir")]
    pub upload_dir: String,

    /// Maximum file size in bytes
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Allowed file extensions
    #[serde(default = "default_allowed_extensions")]
    pub allowed_extensions: Vec<String>,

    /// Organize files by date
    #[serde(default = "default_organize_by_date")]
    pub organize_by_date: bool,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Enable API key authentication
    #[serde(default = "default_enable_auth")]
    pub enable_auth: bool,

    /// Rate limit per minute
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,

    /// Enable CORS
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,

    /// CORS allowed origins
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Require API key for uploads
    #[serde(default = "default_require_api_key")]
    pub require_api_key: bool,

    /// Enable IP-based restrictions
    #[serde(default = "default_enable_ip_restrictions")]
    pub enable_ip_restrictions: bool,

    /// Maximum upload size per request
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size: u64,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format (json or text)
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Log to file
    #[serde(default)]
    pub file: Option<PathBuf>,
}

// Default value functions
fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_port() -> u16 {
    8080
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(4)
}

const fn default_max_connections() -> u32 {
    50
}

const fn default_min_connections() -> u32 {
    5
}

const fn default_connect_timeout() -> u64 {
    30
}

const fn default_idle_timeout() -> u64 {
    600
}

fn default_upload_dir() -> String {
    "uploads".to_string()
}

const fn default_max_file_size() -> u64 {
    100_000_000 // 100MB
}

fn default_allowed_extensions() -> Vec<String> {
    vec!["mp3".to_string(), "wav".to_string(), "flac".to_string()]
}

const fn default_organize_by_date() -> bool {
    true
}

const fn default_enable_auth() -> bool {
    true
}

const fn default_rate_limit() -> u32 {
    60
}

const fn default_enable_cors() -> bool {
    true
}

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

const fn default_require_api_key() -> bool {
    false // Default to false for easier setup
}

const fn default_enable_ip_restrictions() -> bool {
    false
}

const fn default_max_upload_size() -> u64 {
    100_000_000 // 100MB
}

const fn default_request_timeout() -> u64 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

/// Placeholder monitor configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorConfig {
    /// Enable file monitoring
    #[serde(default = "default_monitor_enabled")]
    pub enabled: bool,

    /// Watch directory
    pub watch_directory: Option<PathBuf>,
}

const fn default_monitor_enabled() -> bool {
    false
}

// Re-export TranscriptionConfig from types
pub use crate::types::TranscriptionConfig;

// Default implementation now derived

impl Config {
    /// Load configuration from environment and files
    ///
    /// # Errors
    ///
    /// Returns an error if configuration cannot be loaded or parsed.
    pub fn load() -> crate::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("SDRTRUNK").separator("_"))
            .build()
            .map_err(|e| crate::Error::Configuration {
                message: e.to_string(),
            })?;

        config
            .try_deserialize()
            .map_err(|e| crate::Error::Configuration {
                message: e.to_string(),
            })
    }
}

impl Default for Config {
    fn default() -> Self {
        // Try to get database URL from environment variable, fallback to default
        let database_url = std::env::var("SDRTRUNK_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgresql://localhost/sdrtrunk".to_string());

        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                workers: default_workers(),
            },
            database: DatabaseConfig {
                url: database_url,
                max_connections: default_max_connections(),
                min_connections: default_min_connections(),
                connect_timeout: default_connect_timeout(),
                idle_timeout: default_idle_timeout(),
            },
            storage: StorageConfig {
                base_dir: PathBuf::from(
                    std::env::var("SDRTRUNK_STORAGE_BASE_DIR")
                        .unwrap_or_else(|_| "./data".to_string()),
                ),
                upload_dir: std::env::var("SDRTRUNK_STORAGE_UPLOAD_DIR")
                    .unwrap_or_else(|_| default_upload_dir()),
                max_file_size: default_max_file_size(),
                allowed_extensions: default_allowed_extensions(),
                organize_by_date: default_organize_by_date(),
            },
            api: ApiConfig {
                enable_auth: default_enable_auth(),
                rate_limit: default_rate_limit(),
                enable_cors: default_enable_cors(),
                cors_origins: default_cors_origins(),
            },
            security: SecurityConfig {
                require_api_key: default_require_api_key(),
                enable_ip_restrictions: default_enable_ip_restrictions(),
                max_upload_size: default_max_upload_size(),
                request_timeout: default_request_timeout(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
                format: default_log_format(),
                file: None,
            },
            monitor: None,
            transcription: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
#[allow(
    clippy::unreadable_literal,
    clippy::missing_panics_doc,
    clippy::field_reassign_with_default,
    clippy::absurd_extreme_comparisons,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_config_default() {
        let config = Config::default();

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert!(config.server.workers > 0);

        assert!(config.database.url.contains("postgresql"));
        assert_eq!(config.database.max_connections, 50);
        assert_eq!(config.database.min_connections, 5);

        assert_eq!(config.storage.upload_dir, "uploads");
        assert_eq!(config.storage.max_file_size, 100_000_000);
        assert_eq!(
            config.storage.allowed_extensions,
            vec!["mp3", "wav", "flac"]
        );
        assert!(config.storage.organize_by_date);

        assert!(config.api.enable_auth);
        assert_eq!(config.api.rate_limit, 60);
        assert!(config.api.enable_cors);
        assert_eq!(config.api.cors_origins, vec!["*"]);

        assert!(!config.security.require_api_key);
        assert!(!config.security.enable_ip_restrictions);
        assert_eq!(config.security.max_upload_size, 100_000_000);
        assert_eq!(config.security.request_timeout, 30);

        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "json");
        assert!(config.logging.file.is_none());

        assert!(config.monitor.is_none());
    }

    #[test]
    fn test_server_config() {
        let server_config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
            workers: 4,
        };

        assert_eq!(server_config.host, "127.0.0.1");
        assert_eq!(server_config.port, 3000);
        assert_eq!(server_config.workers, 4);
    }

    #[test]
    fn test_database_config() {
        let db_config = DatabaseConfig {
            url: "postgresql://user:pass@host:5432/db".to_string(),
            max_connections: 100,
            min_connections: 10,
            connect_timeout: 60,
            idle_timeout: 300,
        };

        assert_eq!(db_config.url, "postgresql://user:pass@host:5432/db");
        assert_eq!(db_config.max_connections, 100);
        assert_eq!(db_config.min_connections, 10);
        assert_eq!(db_config.connect_timeout, 60);
        assert_eq!(db_config.idle_timeout, 300);
    }

    #[test]
    fn test_storage_config() {
        let storage_config = StorageConfig {
            base_dir: PathBuf::from("/var/data"),
            upload_dir: "files".to_string(),
            max_file_size: 50_000_000,
            allowed_extensions: vec!["mp3".to_string(), "wav".to_string()],
            organize_by_date: false,
        };

        assert_eq!(storage_config.base_dir, PathBuf::from("/var/data"));
        assert_eq!(storage_config.upload_dir, "files");
        assert_eq!(storage_config.max_file_size, 50_000_000);
        assert_eq!(storage_config.allowed_extensions.len(), 2);
        assert!(!storage_config.organize_by_date);
    }

    #[test]
    fn test_api_config() {
        let api_config = ApiConfig {
            enable_auth: false,
            rate_limit: 100,
            enable_cors: false,
            cors_origins: vec!["https://example.com".to_string()],
        };

        assert!(!api_config.enable_auth);
        assert_eq!(api_config.rate_limit, 100);
        assert!(!api_config.enable_cors);
        assert_eq!(api_config.cors_origins, vec!["https://example.com"]);
    }

    #[test]
    fn test_security_config() {
        let security_config = SecurityConfig {
            require_api_key: true,
            enable_ip_restrictions: true,
            max_upload_size: 200_000_000,
            request_timeout: 60,
        };

        assert!(security_config.require_api_key);
        assert!(security_config.enable_ip_restrictions);
        assert_eq!(security_config.max_upload_size, 200_000_000);
        assert_eq!(security_config.request_timeout, 60);
    }

    #[test]
    fn test_logging_config() {
        let logging_config = LoggingConfig {
            level: "debug".to_string(),
            format: "text".to_string(),
            file: Some(PathBuf::from("/var/log/app.log")),
        };

        assert_eq!(logging_config.level, "debug");
        assert_eq!(logging_config.format, "text");
        assert_eq!(logging_config.file, Some(PathBuf::from("/var/log/app.log")));
    }

    #[test]
    fn test_monitor_config() {
        let monitor_config = MonitorConfig {
            enabled: true,
            watch_directory: Some(PathBuf::from("/watch")),
        };

        assert!(monitor_config.enabled);
        assert_eq!(
            monitor_config.watch_directory,
            Some(PathBuf::from("/watch"))
        );
    }

    #[test]
    fn test_monitor_config_default() {
        let monitor_config = MonitorConfig::default();

        assert!(!monitor_config.enabled);
        assert!(monitor_config.watch_directory.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.server.host, config.server.host);
        assert_eq!(deserialized.server.port, config.server.port);
        assert_eq!(
            deserialized.database.max_connections,
            config.database.max_connections
        );
        assert_eq!(
            deserialized.storage.max_file_size,
            config.storage.max_file_size
        );
        assert_eq!(deserialized.api.rate_limit, config.api.rate_limit);
        assert_eq!(
            deserialized.security.request_timeout,
            config.security.request_timeout
        );
        assert_eq!(deserialized.logging.level, config.logging.level);
    }

    #[test]
    fn test_config_with_monitor() {
        let mut config = Config::default();
        config.monitor = Some(MonitorConfig {
            enabled: true,
            watch_directory: Some(PathBuf::from("/watch")),
        });

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.monitor.is_some());
        let monitor = deserialized.monitor.unwrap();
        assert!(monitor.enabled);
        assert_eq!(monitor.watch_directory, Some(PathBuf::from("/watch")));
    }

    #[test]
    fn test_config_without_monitor() {
        let config = Config::default(); // monitor is None by default

        let serialized = serde_json::to_string(&config).unwrap();

        // When monitor is None, it should not appear in serialized JSON
        assert!(!serialized.contains("monitor"));

        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.monitor.is_none());
    }

    #[test]
    fn test_default_value_functions() {
        assert_eq!(default_host(), "0.0.0.0");
        assert_eq!(default_port(), 8080);
        assert!(default_workers() > 0);
        assert_eq!(default_max_connections(), 50);
        assert_eq!(default_min_connections(), 5);
        assert_eq!(default_connect_timeout(), 30);
        assert_eq!(default_idle_timeout(), 600);
        assert_eq!(default_upload_dir(), "uploads");
        assert_eq!(default_max_file_size(), 100_000_000);
        assert_eq!(default_allowed_extensions(), vec!["mp3", "wav", "flac"]);
        assert!(default_organize_by_date());
        assert!(default_enable_auth());
        assert_eq!(default_rate_limit(), 60);
        assert!(default_enable_cors());
        assert_eq!(default_cors_origins(), vec!["*"]);
        assert!(!default_require_api_key());
        assert!(!default_enable_ip_restrictions());
        assert_eq!(default_max_upload_size(), 100_000_000);
        assert_eq!(default_request_timeout(), 30);
        assert_eq!(default_log_level(), "info");
        assert_eq!(default_log_format(), "json");
        assert!(!default_monitor_enabled());
    }

    #[test]
    fn test_partial_config_deserialization() {
        let json_str = r#"{
            "server": {"host": "localhost"},
            "database": {"url": "postgresql://test"},
            "storage": {"base_dir": "/tmp"},
            "api": {},
            "security": {},
            "logging": {}
        }"#;

        let config: Config = serde_json::from_str(json_str).unwrap();

        assert_eq!(config.server.host, "localhost");
        assert_eq!(config.server.port, 8080); // Uses default
        assert_eq!(config.database.url, "postgresql://test");
        assert_eq!(config.database.max_connections, 50); // Uses default
        assert_eq!(config.storage.base_dir, PathBuf::from("/tmp"));
        assert_eq!(config.storage.upload_dir, "uploads"); // Uses default
    }

    // Note: Environment variable tests removed due to unsafe function restrictions
    // These would require std::env::set_var and std::env::remove_var which are unsafe
    // Integration tests should be used instead to test environment variable handling

    // Note: Database URL fallback tests removed due to unsafe function restrictions
    // These would require std::env::set_var and std::env::remove_var which are unsafe
    // Integration tests should be used instead to test environment variable fallback

    #[test]
    fn test_config_validation_paths() {
        let config = Config::default();

        // Test that paths can be created and are valid
        assert!(config.storage.base_dir.to_str().is_some());
        assert!(!config.storage.upload_dir.is_empty());

        if let Some(log_file) = &config.logging.file {
            assert!(log_file.to_str().is_some());
        }

        if let Some(monitor) = &config.monitor
            && let Some(watch_dir) = &monitor.watch_directory
        {
            assert!(watch_dir.to_str().is_some());
        }
    }

    #[test]
    fn test_config_bounds_validation() {
        let config = Config::default();

        // Test that numeric values are within reasonable bounds
        assert!(config.server.port > 0);
        assert!(config.server.port <= u16::MAX);
        assert!(config.server.workers > 0);
        assert!(config.server.workers < 1000);

        assert!(config.database.max_connections > 0);
        assert!(config.database.max_connections >= config.database.min_connections);
        assert!(config.database.connect_timeout > 0);
        assert!(config.database.idle_timeout > 0);

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
    fn test_complex_config_scenario() {
        let complex_config = Config {
            server: ServerConfig {
                host: "192.168.1.100".to_string(),
                port: 9090,
                workers: 8,
            },
            database: DatabaseConfig {
                url: "postgresql://user:pass@db.example.com:5432/sdrtrunk_prod".to_string(),
                max_connections: 200,
                min_connections: 20,
                connect_timeout: 45,
                idle_timeout: 900,
            },
            storage: StorageConfig {
                base_dir: PathBuf::from("/data/sdrtrunk"),
                upload_dir: "incoming".to_string(),
                max_file_size: 500_000_000, // 500MB
                allowed_extensions: vec![
                    "mp3".to_string(),
                    "wav".to_string(),
                    "flac".to_string(),
                    "m4a".to_string(),
                ],
                organize_by_date: true,
            },
            api: ApiConfig {
                enable_auth: true,
                rate_limit: 120,
                enable_cors: true,
                cors_origins: vec![
                    "https://scanner.example.com".to_string(),
                    "https://admin.example.com".to_string(),
                ],
            },
            security: SecurityConfig {
                require_api_key: true,
                enable_ip_restrictions: true,
                max_upload_size: 500_000_000,
                request_timeout: 120,
            },
            logging: LoggingConfig {
                level: "debug".to_string(),
                format: "json".to_string(),
                file: Some(PathBuf::from("/var/log/sdrtrunk/app.log")),
            },
            monitor: Some(MonitorConfig {
                enabled: true,
                watch_directory: Some(PathBuf::from("/watch/sdrtrunk")),
            }),
            transcription: Some(TranscriptionConfig {
                enabled: true,
                service: "whisperx".to_string(),
                workers: 4,
                queue_size: 200,
                timeout_seconds: 600,
                python_path: Some(PathBuf::from("/opt/whisperx")),
                service_port: Some(9001),
                max_retries: 5,
            }),
        };

        // Test serialization and deserialization of complex config
        let serialized = serde_json::to_string_pretty(&complex_config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.server.host, "192.168.1.100");
        assert_eq!(deserialized.server.port, 9090);
        assert_eq!(deserialized.server.workers, 8);

        assert!(deserialized.database.url.contains("db.example.com"));
        assert_eq!(deserialized.database.max_connections, 200);

        assert_eq!(
            deserialized.storage.base_dir,
            PathBuf::from("/data/sdrtrunk")
        );
        assert_eq!(deserialized.storage.allowed_extensions.len(), 4);

        assert_eq!(deserialized.api.cors_origins.len(), 2);
        assert_eq!(deserialized.api.rate_limit, 120);

        assert!(deserialized.security.require_api_key);
        assert!(deserialized.security.enable_ip_restrictions);

        assert_eq!(deserialized.logging.level, "debug");
        assert!(deserialized.logging.file.is_some());

        assert!(deserialized.monitor.is_some());
        let monitor = deserialized.monitor.unwrap();
        assert!(monitor.enabled);
        assert!(monitor.watch_directory.is_some());
    }
}
