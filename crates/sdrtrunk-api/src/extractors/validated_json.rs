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
#[allow(clippy::missing_panics_doc)]
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

    #[test]
    fn test_validation_edge_cases() {
        // Test minimum valid values
        let min_valid = TestData {
            name: "A".to_string(), // Minimum length
            age: 1,               // Minimum age
            email: None,          // Optional email
        };
        assert!(min_valid.validate().is_ok());
        
        // Test maximum valid values
        let max_valid = TestData {
            name: "1234567890".to_string(), // Maximum length (10 chars)
            age: 100,                      // Maximum age
            email: Some("test@test.co".to_string()),
        };
        assert!(max_valid.validate().is_ok());
        
        // Test empty name (invalid)
        let empty_name = TestData {
            name: "".to_string(),
            age: 25,
            email: None,
        };
        assert!(empty_name.validate().is_err());
        
        // Test zero age (invalid)
        let zero_age = TestData {
            name: "John".to_string(),
            age: 0,
            email: None,
        };
        assert!(zero_age.validate().is_err());
    }

    #[test]
    fn test_validated_json_deref() {
        let data = TestData {
            name: "John".to_string(),
            age: 25,
            email: Some("john@example.com".to_string()),
        };
        
        let validated = ValidatedJson(data);
        
        // Test Deref trait
        assert_eq!(validated.name, "John");
        assert_eq!(validated.age, 25);
        assert_eq!(validated.email, Some("john@example.com".to_string()));
    }

    #[test]
    fn test_validated_json_deref_mut() {
        let data = TestData {
            name: "John".to_string(),
            age: 25,
            email: Some("john@example.com".to_string()),
        };
        
        let mut validated = ValidatedJson(data);
        
        // Test DerefMut trait
        validated.name = "Jane".to_string();
        validated.age = 30;
        
        assert_eq!(validated.name, "Jane");
        assert_eq!(validated.age, 30);
    }

    #[test]
    fn test_email_validation_varieties() {
        let test_cases = vec![
            ("user@example.com", true),
            ("test.email@domain.co.uk", true),
            ("user+tag@example.org", true),
            ("invalid.email", false),
            ("@example.com", false),
            ("user@", false),
            ("", false),
        ];
        
        for (email, should_be_valid) in test_cases {
            let data = TestData {
                name: "Test".to_string(),
                age: 25,
                email: Some(email.to_string()),
            };
            
            let is_valid = data.validate().is_ok();
            assert_eq!(is_valid, should_be_valid, "Email '{}' validation failed", email);
        }
    }

    #[test]
    fn test_multiple_validation_errors() {
        let data = TestData {
            name: "ThisNameIsTooLongToBeValid".to_string(),
            age: 999,
            email: Some("not-an-email".to_string()),
        };
        
        let validation_result = data.validate();
        assert!(validation_result.is_err());
        
        // Should have multiple validation errors
        let errors = validation_result.unwrap_err();
        assert!(!errors.field_errors().is_empty());
    }

    #[test]
    fn test_optional_email_none() {
        let data = TestData {
            name: "John".to_string(),
            age: 25,
            email: None, // None should be valid for optional field
        };
        
        assert!(data.validate().is_ok());
    }

    #[derive(Debug, Serialize, Deserialize, Validate)]
    struct EmptyStruct {}

    #[test]
    fn test_empty_struct_validation() {
        let data = EmptyStruct {};
        assert!(data.validate().is_ok());
    }

    #[derive(Debug, Serialize, Deserialize, Validate)]
    struct NestedData {
        #[validate(length(min = 1))]
        title: String,
        
        #[validate]
        user: TestData,
    }

    #[test]
    fn test_nested_validation() {
        let valid_nested = NestedData {
            title: "Test Title".to_string(),
            user: TestData {
                name: "John".to_string(),
                age: 25,
                email: Some("john@example.com".to_string()),
            },
        };
        assert!(valid_nested.validate().is_ok());
        
        let invalid_nested = NestedData {
            title: "Test Title".to_string(),
            user: TestData {
                name: "".to_string(), // Invalid empty name
                age: 25,
                email: Some("john@example.com".to_string()),
            },
        };
        assert!(invalid_nested.validate().is_err());
    }

    #[derive(Debug, Serialize, Deserialize, Validate)]
    struct CollectionData {
        #[validate(length(min = 1, max = 5))]
        tags: Vec<String>,
        
        #[validate(length(min = 1))]
        #[validate]
        users: Vec<TestData>,
    }

    #[test]
    fn test_collection_validation() {
        let valid_collection = CollectionData {
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            users: vec![
                TestData {
                    name: "John".to_string(),
                    age: 25,
                    email: Some("john@example.com".to_string()),
                },
            ],
        };
        assert!(valid_collection.validate().is_ok());
        
        // Test too many tags
        let too_many_tags = CollectionData {
            tags: vec![
                "tag1".to_string(),
                "tag2".to_string(),
                "tag3".to_string(),
                "tag4".to_string(),
                "tag5".to_string(),
                "tag6".to_string(), // One too many
            ],
            users: vec![
                TestData {
                    name: "John".to_string(),
                    age: 25,
                    email: Some("john@example.com".to_string()),
                },
            ],
        };
        assert!(too_many_tags.validate().is_err());
        
        // Test empty collections
        let empty_collections = CollectionData {
            tags: vec![],
            users: vec![],
        };
        assert!(empty_collections.validate().is_err());
    }
}