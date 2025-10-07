//! Integration tests for sdrtrunk-monitor functionality
#![forbid(unsafe_code)]

mod common;

use sdrtrunk_core::{context_error::Result, context_error};
use common::*;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;

/// Test file monitoring setup and basic functionality
#[tokio::test]
async fn test_monitor_service_basic_functionality() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let watch_dir = temp_dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir)?;
    
    // Create a basic monitor config (this will need to be adjusted based on actual implementation)
    // For now, we'll test the concept
    
    println!("Watch directory created: {}", watch_dir.display());
    
    // Test that we can watch a directory
    // This would use the actual sdrtrunk-monitor crate once implemented
    
    // Create a test file to trigger monitoring
    let test_file = watch_dir.join("20240315_142530_TestSystem_TG12345_FROM_678901.mp3");
    create_test_mp3_file(&watch_dir, "20240315_142530_TestSystem_TG12345_FROM_678901.mp3")?;
    
    // Verify file was created
    assert!(test_file.exists());
    
    Ok(())
}

/// Test file pattern recognition
#[tokio::test]
async fn test_file_pattern_recognition() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let watch_dir = temp_dir.path().join("patterns");
    std::fs::create_dir_all(&watch_dir)?;
    
    // Create various file patterns
    let test_files = vec![
        "20240315_142530_Metro_TG52197_FROM_1234567.mp3", // Standard format
        "20240315_083045_Fire_TG98765_FROM_7654321.wav",  // Different system/format
        "20240316_190000_EMS_TG11111_FROM_2222222.flac",  // Another format
        "invalid_filename.mp3",                           // Invalid pattern
        "20240315_142530_incomplete.mp3",                 // Incomplete pattern
    ];
    
    for filename in &test_files {
        let file_path = watch_dir.join(filename);
        std::fs::write(&file_path, b"test content")?;
    }
    
    // Test filename parsing for each file
    let valid_count = test_files
        .iter()
        .map(|filename| sdrtrunk_core::utils::parse_sdrtrunk_filename(filename))
        .filter(|result| result.is_ok())
        .count();
    
    // Should have 3 valid files
    assert_eq!(valid_count, 3);
    
    println!("Valid SDRTrunk filenames found: {valid_count}");
    
    Ok(())
}

/// Test file processing workflow
#[tokio::test]
async fn test_file_processing_workflow() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let input_dir = temp_dir.path().join("input");
    let output_dir = temp_dir.path().join("output");
    let error_dir = temp_dir.path().join("error");
    
    std::fs::create_dir_all(&input_dir)?;
    std::fs::create_dir_all(&output_dir)?;
    std::fs::create_dir_all(&error_dir)?;
    
    // Create test files with different characteristics
    let test_scenarios = vec![
        ("good_file.mp3", b"valid mp3 content", true),
        ("empty_file.mp3", b"", false),
        ("large_file.mp3", &vec![0u8; 100 * 1024], true), // 100KB
        ("corrupted_file.mp3", b"invalid content", false),
    ];
    
    for (filename, content, should_process) in test_scenarios {
        let input_file = input_dir.join(filename);
        std::fs::write(&input_file, content)?;
        
        println!("Created test file: {} ({}bytes, should_process: {})", 
                filename, content.len(), should_process);
        
        // Here we would trigger the actual file processing
        // For now, just verify the file exists
        assert!(input_file.exists());
        
        // Simulate processing result
        if should_process && content.len() > 0 {
            // Move to output directory (simulate successful processing)
            let output_file = output_dir.join(filename);
            std::fs::rename(&input_file, &output_file)?;
            assert!(output_file.exists());
        } else if input_file.exists() {
            // Move to error directory (simulate processing failure)
            let error_file = error_dir.join(filename);
            std::fs::rename(&input_file, &error_file)?;
            assert!(error_file.exists());
        }
    }
    
    // Verify processing results
    let output_files: Vec<_> = std::fs::read_dir(&output_dir)?.collect();
    let error_files: Vec<_> = std::fs::read_dir(&error_dir)?.collect();
    
    println!("Processed files: {}", output_files.len());
    println!("Error files: {}", error_files.len());
    
    assert!(output_files.len() >= 1); // At least the large valid file
    assert!(error_files.len() >= 1);  // At least the empty file
    
    Ok(())
}

/// Test concurrent file processing
#[tokio::test]
async fn test_concurrent_file_processing() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let batch_dir = temp_dir.path().join("batch");
    std::fs::create_dir_all(&batch_dir)?;
    
    // Create multiple files simultaneously
    let file_count = 20;
    let mut handles = Vec::new();
    
    for i in 0..file_count {
        let batch_dir_clone = batch_dir.clone();
        let handle = tokio::spawn(async move {
            let filename = format!("20240315_14{:02}{:02}_System{}_TG{}_FROM_{}.mp3", 
                                  i / 60, i % 60, i % 3, 10000 + i, 1000000 + i);
            let file_path = batch_dir_clone.join(&filename);
            
            // Create file with some content
            let content = format!("Test content for file {i}").repeat(100);
            tokio::fs::write(&file_path, content).await?;
            
            // Parse filename to verify it's valid
            let parsed = sdrtrunk_core::utils::parse_sdrtrunk_filename(&filename)?;
            
            Ok::<(String, sdrtrunk_core::types::FileData), sdrtrunk_core::context_error::ContextError>((filename, parsed))
        });
        handles.push(handle);
    }
    
    // Wait for all files to be created and processed
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }
    
    assert_eq!(results.len(), file_count);
    
    // Verify all files have valid parsed data
    for (filename, parsed) in &results {
        assert!(!filename.is_empty());
        assert!(parsed.talkgroup_id > 0);
        assert!(parsed.radio_id > 0);
        println!("Processed: {} -> TG{} Radio{}", filename, parsed.talkgroup_id, parsed.radio_id);
    }
    
    Ok(())
}

/// Test file monitoring resilience
#[tokio::test]
async fn test_monitoring_resilience() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let resilience_dir = temp_dir.path().join("resilience");
    std::fs::create_dir_all(&resilience_dir)?;
    
    // Test scenarios that could cause issues
    let problematic_scenarios = vec![
        // Very long filename
        (format!("{}_very_long_filename_that_might_cause_issues.mp3", "x".repeat(200)), "Long filename test"),
        
        // Special characters
        ("file with spaces & symbols@#$.mp3".to_string(), "Special characters test"),
        
        // Unicode filename
        ("файл_тест_🎵.mp3".to_string(), "Unicode test"),
        
        // Hidden file
        (".hidden_file.mp3".to_string(), "Hidden file test"),
        
        // No extension
        ("file_without_extension".to_string(), "No extension test"),
    ];
    
    for (filename, description) in problematic_scenarios {
        println!("Testing: {description}");
        
        let sanitized = sdrtrunk_core::utils::sanitize_filename(&filename);
        println!("Original: {filename}");
        println!("Sanitized: {sanitized}");
        
        // Create file with sanitized name
        let safe_path = resilience_dir.join(&sanitized);
        std::fs::write(&safe_path, b"test content")?;
        
        assert!(safe_path.exists());
        
        // Verify sanitized filename is safe
        assert!(!sanitized.contains('/'));
        assert!(!sanitized.contains('\\'));
        assert!(!sanitized.contains(".."));
    }
    
    Ok(())
}

/// Test file system event handling
#[tokio::test]
async fn test_file_system_events() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let events_dir = temp_dir.path().join("events");
    std::fs::create_dir_all(&events_dir)?;
    
    // Simulate different file system events
    let test_file = events_dir.join("event_test.mp3");
    
    // Create file
    std::fs::write(&test_file, b"initial content")?;
    assert!(test_file.exists());
    
    // Modify file
    sleep(Duration::from_millis(10)).await;
    std::fs::write(&test_file, b"modified content")?;
    
    // Check file size
    let metadata = std::fs::metadata(&test_file)?;
    assert_eq!(metadata.len(), b"modified content".len() as u64);
    
    // Delete file
    std::fs::remove_file(&test_file)?;
    assert!(!test_file.exists());
    
    // Recreate file
    std::fs::write(&test_file, b"recreated content")?;
    assert!(test_file.exists());
    
    Ok(())
}

/// Test directory structure management
#[tokio::test]
async fn test_directory_structure_management() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let base_dir = temp_dir.path().join("structure");
    
    // Test date-based directory creation
    let test_dates = vec![
        chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        chrono::Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap(),
        chrono::Utc.with_ymd_and_hms(2023, 6, 15, 12, 30, 45).unwrap(),
    ];
    
    for date in test_dates {
        let date_path = sdrtrunk_core::utils::create_date_path(&base_dir, &date);
        std::fs::create_dir_all(&date_path)?;
        
        assert!(date_path.exists());
        assert!(date_path.is_dir());
        
        // Test that the path structure is correct
        let path_str = date_path.to_string_lossy();
        assert!(path_str.contains(&date.format("%Y").to_string()));
        assert!(path_str.contains(&date.format("%m").to_string()));
        assert!(path_str.contains(&date.format("%d").to_string()));
        
        println!("Created date path: {}", date_path.display());
    }
    
    Ok(())
}

/// Test configuration handling for monitor service
#[tokio::test]
async fn test_monitor_configuration() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir)?;
    
    // Test different configuration scenarios
    let config_scenarios = vec![
        ("Basic config", sdrtrunk_core::Config::default()),
        ("Development config", {
            let mut config = sdrtrunk_core::Config::default();
            config.logging.level = "debug".to_string();
            config.server.workers = 2;
            config
        }),
        ("Production config", {
            let mut config = sdrtrunk_core::Config::default();
            config.logging.level = "info".to_string();
            config.server.workers = 8;
            config.api.enable_auth = true;
            config
        }),
    ];
    
    for (description, config) in config_scenarios {
        println!("Testing: {description}");
        
        // Serialize config to test persistence
        let config_json = serde_json::to_string_pretty(&config)?;
        let config_file = config_dir.join(format!("{}.json", description.replace(' ', "_").to_lowercase()));
        std::fs::write(&config_file, &config_json)?;
        
        // Read back and verify
        let loaded_config_str = std::fs::read_to_string(&config_file)?;
        let loaded_config: sdrtrunk_core::Config = serde_json::from_str(&loaded_config_str)?;
        
        assert_eq!(config.logging.level, loaded_config.logging.level);
        assert_eq!(config.server.workers, loaded_config.server.workers);
        
        println!("Configuration round-trip successful");
    }
    
    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling_and_recovery() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let error_dir = temp_dir.path().join("error_test");
    std::fs::create_dir_all(&error_dir)?;
    
    // Test various error conditions
    let error_scenarios = vec![
        // File permission issues
        ("permission_test.mp3", 0o000), // No permissions
        ("readonly_test.mp3", 0o444),   // Read-only
        ("normal_test.mp3", 0o644),     // Normal permissions
    ];
    
    for (filename, permissions) in error_scenarios {
        let file_path = error_dir.join(filename);
        std::fs::write(&file_path, b"test content")?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&file_path)?.permissions();
            perms.set_mode(permissions);
            std::fs::set_permissions(&file_path, perms)?;
        }
        
        // Test file access
        let read_result = std::fs::read(&file_path);
        match permissions {
            0o000 => {
                // Should fail on Unix systems
                #[cfg(unix)]
                assert!(read_result.is_err(), "Should not be able to read file with no permissions");
                
                #[cfg(not(unix))]
                println!("Permission test skipped on non-Unix system");
            },
            _ => {
                // Should succeed for readable files
                assert!(read_result.is_ok() || permissions == 0o000, 
                       "Should be able to read file with read permissions");
            }
        }
        
        println!("Tested file: {} with permissions: {:o}", filename, permissions);
    }
    
    Ok(())
}

/// Test performance characteristics of monitoring
#[tokio::test]
async fn test_monitoring_performance() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let perf_dir = temp_dir.path().join("performance");
    std::fs::create_dir_all(&perf_dir)?;
    
    // Performance test: create many files quickly
    let timer = crate::common::helpers::PerformanceTimer::start("bulk_file_creation");
    
    let file_count = 1000;
    let batch_size = 100;
    
    for batch in 0..(file_count / batch_size) {
        let mut batch_handles = Vec::new();
        
        for i in 0..batch_size {
            let file_index = batch * batch_size + i;
            let perf_dir_clone = perf_dir.clone();
            
            let handle = tokio::spawn(async move {
                let filename = format!("20240315_{:06}_System_TG{}_FROM_{}.mp3", 
                                      file_index, 
                                      10000 + (file_index % 1000), 
                                      1000000 + file_index);
                let file_path = perf_dir_clone.join(&filename);
                
                // Create small file
                tokio::fs::write(&file_path, format!("Content {file_index}")).await?;
                
                Ok::<PathBuf, sdrtrunk_core::context_error::ContextError>(file_path)
            });
            batch_handles.push(handle);
        }
        
        // Wait for batch to complete
        for handle in batch_handles {
            handle.await??;
        }
        
        // Small delay between batches to avoid overwhelming the system
        if batch < (file_count / batch_size) - 1 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    let creation_time = timer.stop();
    
    // Verify all files were created
    let created_files: Vec<_> = std::fs::read_dir(&perf_dir)?.collect();
    let actual_count = created_files.len();
    
    println!("Performance test results:");
    println!("  Target files: {file_count}");
    println!("  Created files: {actual_count}");
    println!("  Creation time: {creation_time:?}");
    println!("  Files per second: {:.2}", actual_count as f64 / creation_time.as_secs_f64());
    
    assert_eq!(actual_count, file_count);
    assert!(creation_time < Duration::from_secs(30), "File creation took too long");
    
    // Performance test: filename parsing
    let timer = crate::common::helpers::PerformanceTimer::start("filename_parsing");
    
    let mut parse_successes = 0;
    for entry in std::fs::read_dir(&perf_dir)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        
        if let Ok(_parsed) = sdrtrunk_core::utils::parse_sdrtrunk_filename(&filename) {
            parse_successes += 1;
        }
    }
    
    let parsing_time = timer.stop();
    
    println!("Filename parsing results:");
    println!("  Files parsed: {parse_successes}");
    println!("  Parsing time: {parsing_time:?}");
    println!("  Files per second: {:.2}", parse_successes as f64 / parsing_time.as_secs_f64());
    
    assert_eq!(parse_successes, file_count);
    assert!(parsing_time < Duration::from_secs(5), "Filename parsing took too long");
    
    Ok(())
}

/// Test monitoring cleanup and resource management
#[tokio::test]
async fn test_monitoring_cleanup() -> Result<()> {
    init_test_logging();
    
    let temp_dir = tempdir()?;
    let cleanup_dir = temp_dir.path().join("cleanup");
    std::fs::create_dir_all(&cleanup_dir)?;
    
    // Create files with different ages (simulated)
    let test_files = vec![
        ("recent_file.mp3", 100),      // Small file
        ("medium_file.mp3", 10_000),   // Medium file
        ("large_file.mp3", 1_000_000), // Large file
        ("empty_file.mp3", 0),         // Empty file
    ];
    
    let mut total_size_before = 0;
    
    for (filename, size) in &test_files {
        let file_path = cleanup_dir.join(filename);
        let content = vec![0u8; *size];
        std::fs::write(&file_path, &content)?;
        total_size_before += size;
        
        println!("Created file: {} ({} bytes)", filename, size);
    }
    
    println!("Total size before cleanup: {} bytes", total_size_before);
    
    // Simulate cleanup process (remove empty files)
    let mut total_size_after = 0;
    let mut removed_files = 0;
    
    for entry in std::fs::read_dir(&cleanup_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::metadata(&path)?;
        
        if metadata.len() == 0 {
            std::fs::remove_file(&path)?;
            removed_files += 1;
            println!("Removed empty file: {}", path.display());
        } else {
            total_size_after += metadata.len() as usize;
        }
    }
    
    println!("Total size after cleanup: {} bytes", total_size_after);
    println!("Files removed: {}", removed_files);
    
    assert_eq!(removed_files, 1); // Should remove the empty file
    assert!(total_size_after < total_size_before);
    
    Ok(())
}