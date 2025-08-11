//! Pagination extractor for query parameters

use crate::extractors::ExtractorError;
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Pagination {
    /// Page number (1-based)
    #[validate(range(min = 1, max = 10000))]
    pub page: Option<u32>,
    
    /// Number of items per page
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<u32>,
    
    /// Offset (alternative to page)
    #[validate(range(min = 0))]
    pub offset: Option<u32>,
}

impl Pagination {
    /// Get the effective limit (with default)
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(50).min(1000)
    }
    
    /// Get the effective offset
    pub fn offset(&self) -> u32 {
        if let Some(offset) = self.offset {
            offset
        } else if let Some(page) = self.page {
            (page.saturating_sub(1)) * self.limit()
        } else {
            0
        }
    }
    
    /// Get the current page number (1-based)
    pub fn page(&self) -> u32 {
        if let Some(page) = self.page {
            page
        } else {
            (self.offset() / self.limit()) + 1
        }
    }
    
    /// Check if there's a next page
    pub fn has_next(&self, total_items: u64) -> bool {
        let offset = self.offset() as u64;
        let limit = self.limit() as u64;
        offset + limit < total_items
    }
    
    /// Check if there's a previous page
    pub fn has_prev(&self) -> bool {
        self.offset() > 0
    }
    
    /// Get next page offset
    pub fn next_offset(&self, total_items: u64) -> Option<u32> {
        if self.has_next(total_items) {
            Some(self.offset() + self.limit())
        } else {
            None
        }
    }
    
    /// Get previous page offset
    pub fn prev_offset(&self) -> Option<u32> {
        if self.has_prev() {
            Some(self.offset().saturating_sub(self.limit()))
        } else {
            None
        }
    }
    
    /// Convert to SQL LIMIT/OFFSET values
    pub fn to_sql(&self) -> (i64, i64) {
        (self.limit() as i64, self.offset() as i64)
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(50),
            offset: None,
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Pagination
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts
            .uri
            .query()
            .unwrap_or_default();
        
        let pagination: Pagination = serde_urlencoded::from_str(query)
            .map_err(|e| ExtractorError::bad_request(format!("Invalid pagination parameters: {}", e)))?;
        
        // Validate pagination parameters
        if let Err(validation_errors) = pagination.validate() {
            return Err(ExtractorError::bad_request(format!(
                "Invalid pagination parameters: {:?}", 
                validation_errors
            )));
        }
        
        Ok(pagination)
    }
}

/// Pagination metadata for API responses
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    /// Current page (1-based)
    pub page: u32,
    
    /// Items per page
    pub per_page: u32,
    
    /// Total number of items
    pub total: u64,
    
    /// Total number of pages
    pub total_pages: u32,
    
    /// Whether there's a next page
    pub has_next: bool,
    
    /// Whether there's a previous page
    pub has_prev: bool,
    
    /// Next page number (if exists)
    pub next_page: Option<u32>,
    
    /// Previous page number (if exists)
    pub prev_page: Option<u32>,
}

impl PaginationMeta {
    /// Create pagination metadata
    pub fn new(pagination: &Pagination, total_items: u64) -> Self {
        let page = pagination.page();
        let per_page = pagination.limit();
        let total_pages = ((total_items as f64) / (per_page as f64)).ceil() as u32;
        let has_next = pagination.has_next(total_items);
        let has_prev = pagination.has_prev();
        
        Self {
            page,
            per_page,
            total: total_items,
            total_pages: total_pages.max(1),
            has_next,
            has_prev,
            next_page: if has_next && page < total_pages { Some(page + 1) } else { None },
            prev_page: if has_prev && page > 1 { Some(page - 1) } else { None },
        }
    }
}

/// Paginated response wrapper
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    /// The data items
    pub data: Vec<T>,
    
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

impl<T> PaginatedResponse<T> {
    /// Create a paginated response
    pub fn new(data: Vec<T>, pagination: &Pagination, total_items: u64) -> Self {
        let pagination_meta = PaginationMeta::new(pagination, total_items);
        
        Self {
            data,
            pagination: pagination_meta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, Uri};

    fn create_test_parts_with_query(query: &str) -> Parts {
        let uri: Uri = format!("http://example.com/test?{}", query).parse().unwrap();
        let request = Request::builder().uri(uri).body(()).unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    #[tokio::test]
    async fn test_pagination_extractor_with_page() {
        let mut parts = create_test_parts_with_query("page=2&limit=25");
        let pagination = Pagination::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(pagination.page(), 2);
        assert_eq!(pagination.limit(), 25);
        assert_eq!(pagination.offset(), 25); // (2-1) * 25
    }

    #[tokio::test]
    async fn test_pagination_extractor_with_offset() {
        let mut parts = create_test_parts_with_query("offset=100&limit=50");
        let pagination = Pagination::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(pagination.offset(), 100);
        assert_eq!(pagination.limit(), 50);
        assert_eq!(pagination.page(), 3); // (100 / 50) + 1
    }

    #[tokio::test]
    async fn test_pagination_extractor_defaults() {
        let mut parts = create_test_parts_with_query("");
        let pagination = Pagination::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(pagination.page(), 1);
        assert_eq!(pagination.limit(), 50);
        assert_eq!(pagination.offset(), 0);
    }

    #[tokio::test]
    async fn test_pagination_extractor_validation_error() {
        let mut parts = create_test_parts_with_query("page=0"); // Invalid: page must be >= 1
        let result = Pagination::from_request_parts(&mut parts, &()).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pagination_extractor_max_limit() {
        let mut parts = create_test_parts_with_query("limit=2000"); // Over max
        let result = Pagination::from_request_parts(&mut parts, &()).await;
        
        assert!(result.is_err());
    }

    #[test]
    fn test_pagination_has_next() {
        let pagination = Pagination {
            page: Some(1),
            limit: Some(10),
            offset: None,
        };
        
        assert!(pagination.has_next(100)); // Total 100, showing first 10
        assert!(!pagination.has_next(5)); // Total 5, showing all
    }

    #[test]
    fn test_pagination_has_prev() {
        let pagination1 = Pagination {
            page: Some(1),
            limit: Some(10),
            offset: None,
        };
        
        let pagination2 = Pagination {
            page: Some(2),
            limit: Some(10),
            offset: None,
        };
        
        assert!(!pagination1.has_prev()); // First page
        assert!(pagination2.has_prev()); // Second page
    }

    #[test]
    fn test_pagination_to_sql() {
        let pagination = Pagination {
            page: Some(3),
            limit: Some(20),
            offset: None,
        };
        
        let (limit, offset) = pagination.to_sql();
        assert_eq!(limit, 20);
        assert_eq!(offset, 40); // (3-1) * 20
    }

    #[test]
    fn test_pagination_meta() {
        let pagination = Pagination {
            page: Some(2),
            limit: Some(10),
            offset: None,
        };
        
        let meta = PaginationMeta::new(&pagination, 95);
        
        assert_eq!(meta.page, 2);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total, 95);
        assert_eq!(meta.total_pages, 10); // ceil(95/10)
        assert!(meta.has_next);
        assert!(meta.has_prev);
        assert_eq!(meta.next_page, Some(3));
        assert_eq!(meta.prev_page, Some(1));
    }

    #[test]
    fn test_paginated_response() {
        let data = vec!["item1", "item2", "item3"];
        let pagination = Pagination::default();
        
        let response = PaginatedResponse::new(data, &pagination, 100);
        
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.pagination.total, 100);
        assert_eq!(response.pagination.page, 1);
    }
}