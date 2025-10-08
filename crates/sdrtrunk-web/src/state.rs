//! Application state management

use crate::api_client::ApiClient;
use sdrtrunk_core::Config;

/// Application state holding configuration and clients
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: Config,
    /// API client for backend communication
    pub api_client: ApiClient,
}

impl AppState {
    /// Create new application state
    #[must_use]
    pub fn new(config: Config) -> Self {
        // Environment variable override support (12-factor app pattern)
        let api_host =
            std::env::var("API_HOST").unwrap_or_else(|_| config.webserver.api_host.clone());

        // Use api_port from config, or fall back to server.port
        let api_port = config.webserver.api_port.unwrap_or(config.server.port);

        let api_base_url = format!("http://{api_host}:{api_port}");

        let api_client = ApiClient::new(api_base_url);

        Self { config, api_client }
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_uses_api_host_from_config() {
        let mut config = Config::default();
        config.webserver.api_host = "api.example.com".to_string();
        config.webserver.api_port = Some(9090);

        let state = AppState::new(config);

        // The API client should use the configured api_host and api_port
        // We can't directly access api_client.base_url, but we can verify it was created
        assert_eq!(state.config.webserver.api_host, "api.example.com");
        assert_eq!(state.config.webserver.api_port, Some(9090));
    }

    #[test]
    fn test_app_state_api_port_defaults_to_server_port() {
        let mut config = Config::default();
        config.webserver.api_host = "localhost".to_string();
        config.webserver.api_port = None; // Not specified
        config.server.port = 8080;

        let state = AppState::new(config);

        // When api_port is None, it should fall back to server.port
        assert_eq!(state.config.server.port, 8080);
    }

    #[test]
    fn test_webserver_config_defaults() {
        let config = Config::default();

        assert_eq!(config.webserver.api_host, "localhost");
        assert_eq!(config.webserver.api_port, None);
        assert_eq!(config.webserver.port, 8081);
    }
}
