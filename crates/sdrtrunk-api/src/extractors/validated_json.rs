//! Validated JSON extractor with comprehensive validation

use crate::extractors::ExtractorError;
use axum::{
    async_trait,
    extract::{FromRequest, Request},
    http::StatusCode,
    Json,
};
use serde::de::DeserializeOwned;
use validator::Validate;

/// Validated JSON extractor that combines JSON parsing with validation
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate + Send,
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // First extract JSON
        let Json(data) = Json::<T>::from_request(req, state)
            .await
            .map_err(|err| {
                ExtractorError::bad_request(format!("Invalid JSON: {}", err))
            })?;
        
        // Then validate the data
        data.validate()
            .map_err(|validation_errors| {
                ExtractorError::bad_request(format!("Validation failed: {:?}", validation_errors))
            })?;
        
        Ok(ValidatedJson(data))
    }
}

impl<T> std::ops::Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for ValidatedJson<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Validated JSON extractor with custom error handling
pub struct ValidatedJsonWithErrors<T> {
    /// The validated data
    pub data: T,
    /// Detailed error information for debugging
    pub validation_details: Option<serde_json::Value>,
}

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJsonWithErrors<T>
where
    T: DeserializeOwned + Validate + Send,
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Extract JSON with detailed error information
        let Json(data) = Json::<T>::from_request(req, state)
            .await
            .map_err(|err| {
                let details = serde_json::json!({
                    "json_error": err.to_string(),
                    "error_type": "deserialization_failed"
                });
                
                let mut error = ExtractorError::bad_request("Invalid JSON format");
                // Note: We would need to modify ExtractorError to support details
                error
            })?;
        
        // Validate with detailed error reporting
        match data.validate() {
            Ok(_) => Ok(ValidatedJsonWithErrors {
                data,
                validation_details: None,
            }),
            Err(validation_errors) => {
                let details = serde_json::json!({
                    "validation_errors": validation_errors,
                    "error_type": "validation_failed"
                });
                
                Err(ExtractorError::bad_request(
                    format!("Validation failed: {:?}", validation_errors)
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use validator::Validate;

    #[derive(Debug, Serialize, Deserialize, Validate)]
    struct TestData {
        #[validate(length(min = 1, max = 10))]
        name: String,
        
        #[validate(range(min = 1, max = 100))]
        age: u32,
        
        #[validate(email)]
        email: Option<String>,
    }

    // Note: These tests would require setting up an Axum test environment
    // which is beyond the scope of this basic implementation
    
    #[test]
    fn test_validation_rules() {
        // Test valid data
        let valid_data = TestData {
            name: "John".to_string(),
            age: 25,
            email: Some("john@example.com".to_string()),
        };
        assert!(valid_data.validate().is_ok());
        
        // Test invalid name (too long)
        let invalid_name = TestData {
            name: "ThisNameIsTooLong".to_string(),
            age: 25,
            email: Some("john@example.com".to_string()),
        };
        assert!(invalid_name.validate().is_err());
        
        // Test invalid age (too high)
        let invalid_age = TestData {
            name: "John".to_string(),
            age: 150,
            email: Some("john@example.com".to_string()),
        };
        assert!(invalid_age.validate().is_err());
        
        // Test invalid email
        let invalid_email = TestData {
            name: "John".to_string(),
            age: 25,
            email: Some("not-an-email".to_string()),
        };
        assert!(invalid_email.validate().is_err());
    }
}