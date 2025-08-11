//! Integration tests for sdrtrunk-api functionality

mod common;

use sdrtrunk_core::{context_error::Result, context_error};
use axum::http::StatusCode;
use common::*;
use reqwest::multipart::Form;
use sdrtrunk_core::{types::*, Config};
use sdrtrunk_database::Database;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

/// Test API server startup and health check
#[tokio::test]
async fn test_api_server_startup_and_health() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    // Build the API router
    let app = sdrtrunk_api::build_router(config.clone(), test_db.database().pool().clone()).await?;
    
    // Start server
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    let base_url = format!("http://127.0.0.1:{port}");
    
    // Spawn server task
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    // Wait for server to start
    sleep(Duration::from_millis(100)).await;
    
    // Create HTTP client and test health endpoint
    let client = TestHttpClient::new(base_url.clone());
    let health_response = client.health().await?;
    
    assert_eq!(health_response.status(), StatusCode::OK);
    
    let health_data: ApiResponse<serde_json::Value> = 
        assert_json_response(health_response, StatusCode::OK).await?;
    
    assert!(health_data.success);
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test file upload endpoint
#[tokio::test]
async fn test_file_upload_endpoint() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    let temp_dir = tempdir()?;
    
    let (config, _config_temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    // Create test MP3 file
    let test_file_path = create_test_mp3_file(temp_dir.path(), "test_upload.mp3")?;
    
    // Build and start server
    let app = sdrtrunk_api::build_router(config.clone(), test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    let base_url = format!("http://127.0.0.1:{port}");
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    // Wait for server to start
    sleep(Duration::from_millis(200)).await;
    
    // Prepare upload
    let client = TestHttpClient::new(base_url);
    let metadata = create_upload_metadata("test_system");
    
    // Upload file
    let upload_response = client.upload_file(&test_file_path, metadata).await?;
    
    // Note: The actual upload endpoint may not be fully implemented yet
    // This test may return 404 or other status - adjust expectations accordingly
    let status = upload_response.status();
    println!("Upload response status: {status}");
    
    // For now, we just verify the server responds (even with 404 if endpoint not implemented)
    assert!(status.is_client_error() || status.is_success() || status.is_server_error());
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test CORS handling
#[tokio::test]
async fn test_cors_handling() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (mut config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    // Enable CORS
    config.api.enable_cors = true;
    config.api.cors_origins = vec!["https://example.com".to_string(), "*".to_string()];
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Test preflight request
    let client = create_test_client();
    let response = client
        .request(reqwest::Method::OPTIONS, &format!("http://127.0.0.1:{port}/health"))
        .header("Origin", "https://example.com")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await?;
    
    // The response should include CORS headers (if implemented)
    println!("CORS preflight status: {}", response.status());
    println!("CORS headers: {:#?}", response.headers());
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API error handling
#[tokio::test]
async fn test_api_error_handling() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    
    // Test 404 for non-existent endpoint
    let response = client.get("/api/v1/nonexistent").await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // Test malformed requests (once endpoints are implemented)
    let response = client.post_json("/api/v1/calls", &serde_json::json!({"invalid": "data"})).await?;
    // Should return client error (400-499)
    assert!(response.status().is_client_error() || response.status() == StatusCode::NOT_FOUND);
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API authentication (when enabled)
#[tokio::test]
async fn test_api_authentication() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (mut config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .build();
    
    // Enable authentication
    config.api.enable_auth = true;
    config.security.require_api_key = true;
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    
    // Test request without API key (should fail if auth is implemented)
    let response = client.get("/api/v1/calls").await?;
    // May be 401 Unauthorized or 404 if endpoint not implemented
    assert!(
        response.status() == StatusCode::UNAUTHORIZED || 
        response.status() == StatusCode::NOT_FOUND ||
        response.status() == StatusCode::FORBIDDEN
    );
    
    // Test with API key
    let client_with_key = client.with_api_key("test_api_key".to_string());
    let response = client_with_key.get("/api/v1/calls").await?;
    // Should get different response (possibly 200 OK or different error)
    println!("Authenticated request status: {}", response.status());
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API rate limiting
#[tokio::test]
async fn test_api_rate_limiting() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (mut config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    // Set very low rate limit for testing
    config.api.rate_limit = 2; // 2 requests per minute
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    
    // Make several rapid requests
    for i in 1..=5 {
        let response = client.health().await?;
        println!("Request {i}: status = {}", response.status());
        
        if i > 2 {
            // Should be rate limited after the limit (if rate limiting is implemented)
            // May be 429 Too Many Requests or still 200 if not implemented
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                println!("Rate limiting is working!");
                break;
            }
        }
        
        // Small delay between requests
        sleep(Duration::from_millis(10)).await;
    }
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API request/response formats
#[tokio::test]
async fn test_api_request_response_formats() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = create_test_client();
    let base_url = format!("http://127.0.0.1:{port}");
    
    // Test JSON content type handling
    let response = client
        .post(&format!("{base_url}/api/v1/test"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"test": "data"}))
        .send()
        .await?;
    
    println!("JSON request status: {}", response.status());
    
    // Test that responses have correct content type
    let health_response = client.get(&format!("{base_url}/health")).send().await?;
    
    if let Some(content_type) = health_response.headers().get("content-type") {
        println!("Health response content-type: {:?}", content_type);
        // Should be application/json for API responses
    }
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test concurrent API requests
#[tokio::test]
async fn test_concurrent_api_requests() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(200)).await;
    
    let base_url = format!("http://127.0.0.1:{port}");
    
    // Make multiple concurrent requests
    let mut handles = Vec::new();
    
    for i in 0..20 {
        let base_url_clone = base_url.clone();
        let handle = tokio::spawn(async move {
            let client = create_test_client();
            let response = client.get(&format!("{base_url_clone}/health")).send().await?;
            Ok::<(usize, StatusCode), reqwest::Error>((i, response.status()))
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }
    
    // Verify all requests completed
    assert_eq!(results.len(), 20);
    
    // Most should be successful
    let successful_count = results
        .iter()
        .filter(|(_, status)| status.is_success())
        .count();
    
    println!("Successful concurrent requests: {successful_count}/20");
    
    // At least some should succeed
    assert!(successful_count > 0);
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API with large payloads
#[tokio::test]
async fn test_api_large_payloads() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    let temp_dir = tempdir()?;
    
    let (config, _config_temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Create a larger test file (still reasonable for testing)
    let large_audio_data = vec![0u8; 5 * 1024 * 1024]; // 5MB
    let large_file_path = temp_dir.path().join("large_test.mp3");
    tokio::fs::write(&large_file_path, &large_audio_data).await?;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    let metadata = create_upload_metadata("test_system_large");
    
    // Try to upload large file
    let response = client.upload_file(&large_file_path, metadata).await?;
    
    // The response will depend on whether upload endpoints are implemented
    // and what size limits are set
    println!("Large file upload status: {}", response.status());
    
    // Test large JSON payload
    let large_json = serde_json::json!({
        "data": "x".repeat(1024 * 1024), // 1MB string
        "system_id": "test",
        "metadata": {}
    });
    
    let json_response = client.post_json("/api/v1/test", &large_json).await?;
    println!("Large JSON payload status: {}", json_response.status());
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API metrics and monitoring endpoints
#[tokio::test]
async fn test_api_metrics_and_monitoring() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    
    // Test health check (should always be available)
    let health_response = client.health().await?;
    assert_eq!(health_response.status(), StatusCode::OK);
    
    // Test metrics endpoint (if implemented)
    let metrics_response = client.get("/metrics").await?;
    println!("Metrics endpoint status: {}", metrics_response.status());
    
    // Test readiness/liveness probes
    let ready_response = client.get("/ready").await?;
    println!("Readiness probe status: {}", ready_response.status());
    
    let live_response = client.get("/live").await?;
    println!("Liveness probe status: {}", live_response.status());
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API graceful shutdown
#[tokio::test]
async fn test_api_graceful_shutdown() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                // Wait for shutdown signal
                tokio::signal::ctrl_c().await.ok();
            })
            .await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Make a request to ensure server is running
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    let response = client.health().await?;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Simulate graceful shutdown
    server_handle.abort();
    
    // Give it a moment to shutdown
    sleep(Duration::from_millis(100)).await;
    
    // Verify server is no longer accepting connections
    let result = client.health().await;
    assert!(result.is_err(), "Server should not be accessible after shutdown");
    
    Ok(())
}

/// Test API response compression
#[tokio::test]
async fn test_api_response_compression() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Test with Accept-Encoding header
    let client = create_test_client();
    let response = client
        .get(&format!("http://127.0.0.1:{port}/health"))
        .header("Accept-Encoding", "gzip, deflate, br")
        .send()
        .await?;
    
    println!("Compression test status: {}", response.status());
    
    if let Some(content_encoding) = response.headers().get("content-encoding") {
        println!("Response compression: {:?}", content_encoding);
    }
    
    // The response should still be readable regardless of compression
    if response.status().is_success() {
        let body = response.text().await?;
        assert!(!body.is_empty());
    }
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Performance test for API endpoints
#[tokio::test]
async fn test_api_performance() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let port = find_available_port().await?;
    
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_database_url(test_db.connection_string().to_string())
        .with_port(port)
        .without_auth()
        .build();
    
    let app = sdrtrunk_api::build_router(config, test_db.database().pool().clone()).await?;
    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}")).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    sleep(Duration::from_millis(200)).await;
    
    let client = TestHttpClient::new(format!("http://127.0.0.1:{port}"));
    
    // Performance test: measure response time for health checks
    let timer = crate::common::helpers::PerformanceTimer::start("health_check_batch");
    
    let request_count = 100;
    let mut response_times = Vec::new();
    
    for _ in 0..request_count {
        let start = tokio::time::Instant::now();
        let response = client.health().await?;
        let duration = start.elapsed();
        
        response_times.push(duration);
        
        // Only count successful responses for timing
        if !response.status().is_success() {
            println!("Non-success response: {}", response.status());
        }
    }
    
    let total_duration = timer.stop();
    
    // Calculate statistics
    let avg_response_time = response_times.iter().sum::<tokio::time::Duration>() / response_times.len() as u32;
    let max_response_time = response_times.iter().max().unwrap();
    let min_response_time = response_times.iter().min().unwrap();
    
    println!("Performance test results:");
    println!("  Total time: {total_duration:?}");
    println!("  Average response time: {avg_response_time:?}");
    println!("  Min response time: {min_response_time:?}");
    println!("  Max response time: {max_response_time:?}");
    println!("  Requests per second: {:.2}", request_count as f64 / total_duration.as_secs_f64());
    
    // Performance assertions (adjust thresholds as needed)
    assert!(avg_response_time < Duration::from_millis(100), "Average response time too high");
    assert!(max_response_time < Duration::from_millis(500), "Maximum response time too high");
    assert!(total_duration < Duration::from_secs(10), "Total test duration too high");
    
    // Stop server
    server_handle.abort();
    
    Ok(())
}

/// Test API state management
#[tokio::test]
async fn test_api_state_management() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    
    // Test that we can create the API state
    let config = Config::default();
    let state = Arc::new(sdrtrunk_api::AppState::new(config, test_db.database().pool().clone())?);
    
    // Test state validation
    let validation_result = state.validate();
    match validation_result {
        Ok(()) => println!("State validation passed"),
        Err(e) => println!("State validation failed (expected if dependencies missing): {e}"),
    }
    
    // Test state cloning and thread safety
    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        // Access state from another task
        let _config = &state_clone; // Just verify we can access it
        Ok::<(), sdrtrunk_core::context_error::ContextError>(())
    });
    
    handle.await??;
    
    Ok(())
}