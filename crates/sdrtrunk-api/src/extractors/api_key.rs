//! API key extractor for authenticated requests

use crate::extractors::ExtractorError;
use axum::{
    async_trait,
    extract::{FromRequestParts, Request},
    http::request::Parts,
};
use sdrtrunk_database::models::ApiKeyDb;

/// Extractor for API key information from authenticated requests
#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    /// The authenticated API key
    pub api_key: ApiKeyDb,
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyInfo
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get API key from request extensions (added by auth middleware)
        let api_key = parts
            .extensions
            .get::<ApiKeyDb>()
            .cloned()
            .ok_or_else(|| {
                ExtractorError::unauthorized("Valid API key required")
            })?;
        
        Ok(ApiKeyInfo { api_key })
    }
}

impl ApiKeyInfo {
    /// Get the API key ID
    pub fn key_id(&self) -> &str {
        &self.api_key.id
    }
    
    /// Get the API key description
    pub fn description(&self) -> Option<&str> {
        self.api_key.description.as_deref()
    }
    
    /// Check if the API key allows access to a specific system
    pub fn can_access_system(&self, system_id: &str) -> bool {
        // If no system restrictions, allow all
        if let Some(allowed_systems) = &self.api_key.allowed_systems {
            if allowed_systems.is_empty() {
                return true;
            }
            return allowed_systems.contains(&system_id.to_string());
        }
        true
    }
    
    /// Check if the API key allows access from a specific IP
    pub fn can_access_from_ip(&self, ip: &str) -> bool {
        // If no IP restrictions, allow all
        if let Some(allowed_ips) = &self.api_key.allowed_ips {
            if allowed_ips.is_empty() {
                return true;
            }
            // Basic IP matching - in production you might want CIDR support
            return allowed_ips.contains(&ip.to_string());
        }
        true
    }
    
    /// Get the total number of requests made with this key
    pub fn total_requests(&self) -> i32 {
        self.api_key.total_requests.unwrap_or(0)
    }
    
    /// Check if the API key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.api_key.expires_at {
            return expires_at < chrono::Utc::now();
        }
        false
    }
    
    /// Get time until expiration (if applicable)
    pub fn time_until_expiration(&self) -> Option<chrono::Duration> {
        self.api_key.expires_at.map(|expires_at| {
            expires_at - chrono::Utc::now()
        })
    }
}

/// Optional API key extractor that doesn't fail if no API key is present
#[derive(Debug, Clone)]
pub struct OptionalApiKeyInfo(pub Option<ApiKeyInfo>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalApiKeyInfo
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match ApiKeyInfo::from_request_parts(parts, state).await {
            Ok(api_key_info) => Ok(OptionalApiKeyInfo(Some(api_key_info))),
            Err(_) => Ok(OptionalApiKeyInfo(None)),
        }
    }
}

impl OptionalApiKeyInfo {
    /// Check if an API key is present
    pub fn is_authenticated(&self) -> bool {
        self.0.is_some()
    }
    
    /// Get the API key info if present
    pub fn api_key(&self) -> Option<&ApiKeyInfo> {
        self.0.as_ref()
    }
    
    /// Convert to required API key info, returning error if not present
    pub fn require(self) -> Result<ApiKeyInfo, ExtractorError> {
        self.0.ok_or_else(|| ExtractorError::unauthorized("API key required"))
    }
}

/// API key permissions checker
#[derive(Debug)]
pub struct ApiKeyPermissions {
    /// The API key information
    pub api_key: ApiKeyInfo,
    /// Required system access
    pub required_system: Option<String>,
    /// Client IP address
    pub client_ip: Option<String>,
}

impl ApiKeyPermissions {
    /// Create a new permissions checker
    pub fn new(api_key: ApiKeyInfo) -> Self {
        Self {
            api_key,
            required_system: None,
            client_ip: None,
        }
    }
    
    /// Set required system access
    pub fn require_system_access(mut self, system_id: impl Into<String>) -> Self {
        self.required_system = Some(system_id.into());
        self
    }
    
    /// Set client IP for IP-based restrictions
    pub fn with_client_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
    }
    
    /// Check if all permissions are satisfied
    pub fn check(&self) -> Result<(), ExtractorError> {
        // Check system access if required
        if let Some(required_system) = &self.required_system {
            if !self.api_key.can_access_system(required_system) {
                return Err(ExtractorError::new(
                    format!("API key does not have access to system: {}", required_system),
                    axum::http::StatusCode::FORBIDDEN,
                    "INSUFFICIENT_SYSTEM_ACCESS",
                ));
            }
        }
        
        // Check IP access if client IP is provided
        if let Some(client_ip) = &self.client_ip {
            if !self.api_key.can_access_from_ip(client_ip) {
                return Err(ExtractorError::new(
                    format!("API key does not allow access from IP: {}", client_ip),
                    axum::http::StatusCode::FORBIDDEN,
                    "IP_ACCESS_DENIED",
                ));
            }
        }
        
        // Check if key is expired
        if self.api_key.is_expired() {
            return Err(ExtractorError::new(
                "API key has expired",
                axum::http::StatusCode::UNAUTHORIZED,
                "API_KEY_EXPIRED",
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn create_test_api_key() -> ApiKeyDb {
        ApiKeyDb {
            id: "test-key".to_string(),
            key_hash: "hash123".to_string(),
            description: Some("Test API Key".to_string()),
            created_at: Utc::now(),
            expires_at: None,
            allowed_ips: Some(vec!["192.168.1.1".to_string()]),
            allowed_systems: Some(vec!["system1".to_string(), "system2".to_string()]),
            active: true,
            last_used: None,
            total_requests: Some(100),
        }
    }

    #[test]
    fn test_api_key_info_system_access() {
        let api_key_info = ApiKeyInfo {
            api_key: create_test_api_key(),
        };
        
        assert!(api_key_info.can_access_system("system1"));
        assert!(api_key_info.can_access_system("system2"));
        assert!(!api_key_info.can_access_system("system3"));
    }

    #[test]
    fn test_api_key_info_ip_access() {
        let api_key_info = ApiKeyInfo {
            api_key: create_test_api_key(),
        };
        
        assert!(api_key_info.can_access_from_ip("192.168.1.1"));
        assert!(!api_key_info.can_access_from_ip("192.168.1.2"));
    }

    #[test]
    fn test_api_key_permissions() {
        let api_key_info = ApiKeyInfo {
            api_key: create_test_api_key(),
        };
        
        let permissions = ApiKeyPermissions::new(api_key_info)
            .require_system_access("system1")
            .with_client_ip("192.168.1.1");
        
        assert!(permissions.check().is_ok());
        
        // Test with disallowed system
        let api_key_info = ApiKeyInfo {
            api_key: create_test_api_key(),
        };
        
        let permissions = ApiKeyPermissions::new(api_key_info)
            .require_system_access("system3");
        
        assert!(permissions.check().is_err());
    }

    #[test]
    fn test_expired_api_key() {
        let mut api_key = create_test_api_key();
        api_key.expires_at = Some(Utc::now() - chrono::Duration::hours(1)); // Expired 1 hour ago
        
        let api_key_info = ApiKeyInfo { api_key };
        assert!(api_key_info.is_expired());
        
        let permissions = ApiKeyPermissions::new(api_key_info);
        assert!(permissions.check().is_err());
    }
}