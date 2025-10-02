//! Application state management

use sdrtrunk_core::Config;
use crate::api_client::ApiClient;

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
    pub fn new(config: Config) -> Self {
        let api_base_url = format!("http://{}:{}", config.server.host, config.server.port);
        let api_client = ApiClient::new(api_base_url);

        Self { config, api_client }
    }
}