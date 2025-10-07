//! HTTP client for communicating with the `SDRTrunk` API

use reqwest::Client;
use sdrtrunk_core::Result;

// Import actual types from API handlers
pub use sdrtrunk_api::handlers::calls::{CallSummary, ListCallsQuery, ListCallsResponse, PaginationInfo};
pub use sdrtrunk_api::handlers::stats::{GlobalStatsResponse, SystemSummary, ActivityPeriod, StorageStats};

/// API client for making HTTP requests to the `SDRTrunk` API server
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: None,
        }
    }

    /// Set the API key for authentication
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Get a list of radio calls with optional filtering
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn get_calls(&self, params: &ListCallsQuery) -> Result<serde_json::Value> {
        let mut url = format!("{}/api/calls", self.base_url);

        // Add query parameters
        let mut query_params = Vec::new();

        if let Some(limit) = params.limit {
            query_params.push(format!("limit={limit}"));
        }
        if let Some(offset) = params.offset {
            query_params.push(format!("offset={offset}"));
        }
        if let Some(ref system_id) = params.system_id {
            query_params.push(format!("system_id={}", urlencoding::encode(system_id)));
        }
        if let Some(talkgroup_id) = params.talkgroup_id {
            query_params.push(format!("talkgroup_id={talkgroup_id}"));
        }
        if let Some(ref from_date) = params.from_date {
            query_params.push(format!("from_date={}", urlencoding::encode(&from_date.to_rfc3339())));
        }
        if let Some(ref to_date) = params.to_date {
            query_params.push(format!("to_date={}", urlencoding::encode(&to_date.to_rfc3339())));
        }
        if let Some(ref sort) = params.sort {
            query_params.push(format!("sort={}", urlencoding::encode(sort)));
        }
        if let Some(include_transcription) = params.include_transcription {
            query_params.push(format!("include_transcription={include_transcription}"));
        }

        if !query_params.is_empty() {
            url.push('?');
            url.push_str(&query_params.join("&"));
        }

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request.send().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to fetch calls: {e}")))?;

        if !response.status().is_success() {
            return Err(sdrtrunk_core::Error::Other(format!(
                "API returned error: {}",
                response.status()
            )));
        }

        let calls_response: serde_json::Value = response.json().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to parse response: {e}")))?;

        Ok(calls_response)
    }

    /// Get system statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn get_system_stats(&self, system_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/systems/{}/stats", self.base_url, system_id);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request.send().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to fetch system stats: {e}")))?;

        if !response.status().is_success() {
            return Err(sdrtrunk_core::Error::Other(format!(
                "API returned error: {}",
                response.status()
            )));
        }

        let stats: serde_json::Value = response.json().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to parse system stats: {e}")))?;

        Ok(stats)
    }

    /// Get details for a specific call
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn get_call_details(&self, call_id: uuid::Uuid) -> Result<serde_json::Value> {
        let url = format!("{}/api/calls/{}", self.base_url, call_id);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request.send().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to fetch call details: {e}")))?;

        if !response.status().is_success() {
            return Err(sdrtrunk_core::Error::Other(format!(
                "Call not found: {}",
                response.status()
            )));
        }

        let call_data: serde_json::Value = response.json().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to parse call details: {e}")))?;

        Ok(call_data)
    }

    /// Get global statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn get_global_stats(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/stats/global", self.base_url);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request.send().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to fetch global stats: {e}")))?;

        if !response.status().is_success() {
            return Err(sdrtrunk_core::Error::Other(format!(
                "API returned error: {}",
                response.status()
            )));
        }

        let stats: serde_json::Value = response.json().await
            .map_err(|e| sdrtrunk_core::Error::Other(format!("Failed to parse global stats: {e}")))?;

        Ok(stats)
    }
}


