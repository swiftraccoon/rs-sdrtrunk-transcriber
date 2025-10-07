//! Integration tests for sdrtrunk-database functionality
#![forbid(unsafe_code)]

mod common;

use sdrtrunk_core::context_error::Result;
use common::*;
use sdrtrunk_core::{types::*, Config};
use sdrtrunk_database::{models::*, queries::*, Database};
use sqlx::{Row, Executor};
use std::collections::HashMap;
use testcontainers::{clients, runners::AsyncRunner, Image};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

/// Test database connection and health check
#[tokio::test]
async fn test_database_connection_and_health() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Test health check
    let health_result = db.health_check().await;
    assert!(health_result.is_ok(), "Health check should pass: {health_result:?}");
    
    // Test direct query
    let row = sqlx::query("SELECT 1 as test_value")
        .fetch_one(db.pool())
        .await?;
    
    let test_value: i32 = row.get("test_value");
    assert_eq!(test_value, 1);
    
    Ok(())
}

/// Test database migrations
#[tokio::test]
async fn test_database_migrations() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Migrations should have been applied during setup
    // Test that expected tables exist
    let tables_query = r#"
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema = 'public'
        ORDER BY table_name
    "#;
    
    let rows = sqlx::query(tables_query)
        .fetch_all(db.pool())
        .await?;
    
    let table_names: Vec<String> = rows
        .iter()
        .map(|row| row.get::<String, _>("table_name"))
        .collect();
    
    // Check for expected tables (these should be created by migrations)
    let expected_tables = vec![
        "_sqlx_migrations",  // SQLx migration tracking table
    ];
    
    for expected_table in expected_tables {
        assert!(
            table_names.contains(&expected_table.to_string()),
            "Expected table '{expected_table}' not found. Available tables: {table_names:?}"
        );
    }
    
    Ok(())
}

/// Test radio call database operations
#[tokio::test]
async fn test_radio_call_database_operations() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    // TestDatabase::new() already runs migrations from
    // crates/sdrtrunk-database/migrations/
    // Verify the migration created the radio_calls table correctly
    let table_check_sql = r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'radio_calls'
    "#;

    let row = sqlx::query(table_check_sql)
        .fetch_one(db.pool())
        .await?;

    let table_name: String = row.get("table_name");
    assert_eq!(
        table_name,
        "radio_calls",
        "Migration should create radio_calls table"
    );
    
    // Test INSERT
    let call_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    
    let insert_sql = r#"
        INSERT INTO radio_calls (
            id, created_at, call_timestamp, system_id, system_label, 
            talkgroup_id, talkgroup_label, frequency, source_radio_id,
            transcription_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    "#;
    
    sqlx::query(insert_sql)
        .bind(call_id)
        .bind(now)
        .bind(now)
        .bind("test_system")
        .bind("Test System")
        .bind(12345i32)
        .bind("Test Talkgroup")
        .bind(854000000i64)
        .bind(678901i32)
        .bind("none")
        .execute(db.pool())
        .await?;
    
    // Test SELECT
    let select_sql = "SELECT * FROM radio_calls WHERE id = $1";
    let row = sqlx::query(select_sql)
        .bind(call_id)
        .fetch_one(db.pool())
        .await?;
    
    let retrieved_id: Uuid = row.get("id");
    let system_id: String = row.get("system_id");
    let talkgroup_id: Option<i32> = row.get("talkgroup_id");
    
    assert_eq!(retrieved_id, call_id);
    assert_eq!(system_id, "test_system");
    assert_eq!(talkgroup_id, Some(12345));
    
    // Test UPDATE
    let update_sql = r#"
        UPDATE radio_calls 
        SET transcription_status = $1, transcription_text = $2 
        WHERE id = $3
    "#;
    
    sqlx::query(update_sql)
        .bind("completed")
        .bind("Test transcription text")
        .bind(call_id)
        .execute(db.pool())
        .await?;
    
    // Verify update
    let row = sqlx::query(select_sql)
        .bind(call_id)
        .fetch_one(db.pool())
        .await?;
    
    let transcription_status: Option<String> = row.get("transcription_status");
    let transcription_text: Option<String> = row.get("transcription_text");
    
    assert_eq!(transcription_status, Some("completed".to_string()));
    assert_eq!(transcription_text, Some("Test transcription text".to_string()));
    
    // Test DELETE
    let delete_sql = "DELETE FROM radio_calls WHERE id = $1";
    let result = sqlx::query(delete_sql)
        .bind(call_id)
        .execute(db.pool())
        .await?;
    
    assert_eq!(result.rows_affected(), 1);
    
    // Verify deletion
    let count_sql = "SELECT COUNT(*) as count FROM radio_calls WHERE id = $1";
    let row = sqlx::query(count_sql)
        .bind(call_id)
        .fetch_one(db.pool())
        .await?;
    
    let count: i64 = row.get("count");
    assert_eq!(count, 0);
    
    Ok(())
}

/// Test complex JSON field operations
#[tokio::test]
async fn test_json_field_operations() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create table with JSONB fields
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_json (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            metadata JSONB,
            patches JSONB,
            frequencies JSONB,
            speaker_segments JSONB
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Test complex JSON data
    let metadata = serde_json::json!({
        "system": "metro_police",
        "location": {
            "lat": 40.7128,
            "lon": -74.0060,
            "address": "New York, NY"
        },
        "tags": ["emergency", "police", "dispatch"]
    });
    
    let patches = serde_json::json!([
        {"id": 1, "name": "North District", "active": true},
        {"id": 2, "name": "South District", "active": false}
    ]);
    
    let frequencies = serde_json::json!([854000000, 855000000, 856000000]);
    
    let speaker_segments = serde_json::json!([
        {
            "speaker": 0,
            "start": 0.0,
            "end": 15.5,
            "confidence": 0.98,
            "text": "Unit 23 responding"
        },
        {
            "speaker": 1,
            "start": 16.0,
            "end": 30.0,
            "confidence": 0.92,
            "text": "Copy that, Unit 23"
        }
    ]);
    
    // Insert JSON data
    let insert_sql = r#"
        INSERT INTO test_json (metadata, patches, frequencies, speaker_segments)
        VALUES ($1, $2, $3, $4)
        RETURNING id
    "#;
    
    let row = sqlx::query(insert_sql)
        .bind(&metadata)
        .bind(&patches)
        .bind(&frequencies)
        .bind(&speaker_segments)
        .fetch_one(db.pool())
        .await?;
    
    let record_id: Uuid = row.get("id");
    
    // Query back the JSON data
    let select_sql = "SELECT * FROM test_json WHERE id = $1";
    let row = sqlx::query(select_sql)
        .bind(record_id)
        .fetch_one(db.pool())
        .await?;
    
    let retrieved_metadata: serde_json::Value = row.get("metadata");
    let retrieved_patches: serde_json::Value = row.get("patches");
    let retrieved_frequencies: serde_json::Value = row.get("frequencies");
    let retrieved_speaker_segments: serde_json::Value = row.get("speaker_segments");
    
    assert_eq!(retrieved_metadata, metadata);
    assert_eq!(retrieved_patches, patches);
    assert_eq!(retrieved_frequencies, frequencies);
    assert_eq!(retrieved_speaker_segments, speaker_segments);
    
    // Test JSON queries
    let json_query_sql = r#"
        SELECT id FROM test_json 
        WHERE metadata->>'system' = 'metro_police'
    "#;
    
    let rows = sqlx::query(json_query_sql)
        .fetch_all(db.pool())
        .await?;
    
    assert_eq!(rows.len(), 1);
    
    // Test JSON array queries
    let array_query_sql = r#"
        SELECT id FROM test_json 
        WHERE frequencies @> '[854000000]'
    "#;
    
    let rows = sqlx::query(array_query_sql)
        .fetch_all(db.pool())
        .await?;
    
    assert_eq!(rows.len(), 1);
    
    Ok(())
}

/// Test database connection pool behavior
#[tokio::test]
async fn test_connection_pool_behavior() -> Result<()> {
    init_test_logging();
    
    let docker = clients::Cli::default();
    let postgres = Postgres::default().with_tag("16-alpine");
    let container = postgres.start(&docker).await?;
    
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    
    let mut config = Config::default();
    config.database.url = format!("postgresql://postgres:postgres@{host}:{port}/postgres");
    config.database.max_connections = 5;
    config.database.min_connections = 2;
    
    let db = Database::new(&config).await?;
    db.migrate().await?;
    
    // Test that we can create multiple connections
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            // Each task performs a simple query
            let query = "SELECT $1::integer as value";
            let row = sqlx::query(query)
                .bind(i)
                .fetch_one(db_clone.pool())
                .await?;
            
            let value: i32 = row.get("value");
            Ok::<i32, sqlx::Error>(value)
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await??;
        results.push(result);
    }
    
    // Verify all queries completed successfully
    assert_eq!(results.len(), 10);
    for (i, &result) in results.iter().enumerate() {
        assert_eq!(result, i as i32);
    }
    
    Ok(())
}

/// Test transaction handling
#[tokio::test]
async fn test_transaction_handling() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create test table
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_transactions (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            value INTEGER NOT NULL
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Test successful transaction
    let mut tx = db.pool().begin().await?;
    
    sqlx::query("INSERT INTO test_transactions (name, value) VALUES ($1, $2)")
        .bind("test1")
        .bind(100)
        .execute(&mut *tx)
        .await?;
    
    sqlx::query("INSERT INTO test_transactions (name, value) VALUES ($1, $2)")
        .bind("test2")
        .bind(200)
        .execute(&mut *tx)
        .await?;
    
    tx.commit().await?;
    
    // Verify data was committed
    let count_sql = "SELECT COUNT(*) as count FROM test_transactions";
    let row = sqlx::query(count_sql)
        .fetch_one(db.pool())
        .await?;
    
    let count: i64 = row.get("count");
    assert_eq!(count, 2);
    
    // Test rollback transaction
    let mut tx = db.pool().begin().await?;
    
    sqlx::query("INSERT INTO test_transactions (name, value) VALUES ($1, $2)")
        .bind("test3")
        .bind(300)
        .execute(&mut *tx)
        .await?;
    
    // Simulate an error or explicit rollback
    tx.rollback().await?;
    
    // Verify data was not committed
    let row = sqlx::query(count_sql)
        .fetch_one(db.pool())
        .await?;
    
    let count: i64 = row.get("count");
    assert_eq!(count, 2); // Should still be 2, not 3
    
    Ok(())
}

/// Test database error handling
#[tokio::test]
async fn test_database_error_handling() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Test query on non-existent table
    let result = sqlx::query("SELECT * FROM non_existent_table")
        .fetch_one(db.pool())
        .await;
    
    assert!(result.is_err());
    
    // Test invalid SQL syntax
    let result = sqlx::query("INVALID SQL SYNTAX")
        .fetch_one(db.pool())
        .await;
    
    assert!(result.is_err());
    
    // Test constraint violation (if we had constraints)
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_constraints (
            id SERIAL PRIMARY KEY,
            unique_name VARCHAR(100) UNIQUE NOT NULL
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Insert first record
    sqlx::query("INSERT INTO test_constraints (unique_name) VALUES ($1)")
        .bind("unique_value")
        .execute(db.pool())
        .await?;
    
    // Try to insert duplicate
    let result = sqlx::query("INSERT INTO test_constraints (unique_name) VALUES ($1)")
        .bind("unique_value")
        .execute(db.pool())
        .await;
    
    assert!(result.is_err());
    
    Ok(())
}

/// Test database models serialization/deserialization
#[tokio::test]
async fn test_database_models() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create comprehensive test table matching RadioCallDb structure
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_radio_calls (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            call_timestamp TIMESTAMPTZ NOT NULL,
            system_id VARCHAR(50) NOT NULL,
            system_label VARCHAR(255),
            frequency BIGINT,
            talkgroup_id INTEGER,
            talkgroup_label VARCHAR(255),
            talkgroup_group VARCHAR(255),
            talkgroup_tag VARCHAR(255),
            source_radio_id INTEGER,
            talker_alias VARCHAR(255),
            audio_filename VARCHAR(255),
            audio_file_path TEXT,
            audio_size_bytes BIGINT,
            audio_content_type VARCHAR(100),
            duration_seconds DECIMAL(10, 3),
            transcription_text TEXT,
            transcription_confidence DECIMAL(5, 4),
            transcription_language VARCHAR(10),
            transcription_status VARCHAR(20),
            speaker_segments JSONB,
            speaker_count INTEGER,
            patches TEXT,
            frequencies TEXT,
            sources TEXT,
            upload_ip INET,
            upload_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            upload_api_key_id VARCHAR(255)
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Create a comprehensive test record
    let call_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let speaker_segments = serde_json::json!([
        {"speaker": 0, "start": 0.0, "end": 30.5, "confidence": 0.95}
    ]);
    
    let insert_sql = r#"
        INSERT INTO test_radio_calls (
            id, created_at, call_timestamp, system_id, system_label,
            frequency, talkgroup_id, talkgroup_label, talkgroup_group, talkgroup_tag,
            source_radio_id, talker_alias, audio_filename, audio_file_path,
            audio_size_bytes, audio_content_type, duration_seconds,
            transcription_text, transcription_confidence, transcription_language,
            transcription_status, speaker_segments, speaker_count,
            patches, frequencies, sources, upload_ip, upload_timestamp, upload_api_key_id
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29
        )
    "#;
    
    sqlx::query(insert_sql)
        .bind(call_id)
        .bind(now)
        .bind(now - chrono::Duration::minutes(5))
        .bind("metro_police")
        .bind("Metro Police Department")
        .bind(854000000i64)
        .bind(52197i32)
        .bind("Police Dispatch")
        .bind("Operations")
        .bind("Law Enforcement")
        .bind(1234567i32)
        .bind("Unit 23")
        .bind("test_audio.mp3")
        .bind("/data/2024/03/15/test_audio.mp3")
        .bind(1024000i64)
        .bind("audio/mpeg")
        .bind(rust_decimal::Decimal::try_from(30.5).unwrap())
        .bind("Unit 23 responding to traffic stop")
        .bind(rust_decimal::Decimal::try_from(0.95).unwrap())
        .bind("en")
        .bind("completed")
        .bind(speaker_segments)
        .bind(1i32)
        .bind("patch_data")
        .bind("frequency_data")
        .bind("source_data")
        .bind("192.168.1.100".parse::<std::net::IpAddr>()?)
        .bind(now)
        .bind("test_api_key")
        .execute(db.pool())
        .await?;
    
    // Query back using FromRow derivation
    let select_sql = "SELECT * FROM test_radio_calls WHERE id = $1";
    
    // Test manual field access
    let row = sqlx::query(select_sql)
        .bind(call_id)
        .fetch_one(db.pool())
        .await?;
    
    let retrieved_id: Uuid = row.get("id");
    let system_id: String = row.get("system_id");
    let talkgroup_id: Option<i32> = row.get("talkgroup_id");
    let duration: Option<rust_decimal::Decimal> = row.get("duration_seconds");
    let speaker_segments: Option<serde_json::Value> = row.get("speaker_segments");
    
    assert_eq!(retrieved_id, call_id);
    assert_eq!(system_id, "metro_police");
    assert_eq!(talkgroup_id, Some(52197));
    assert!(duration.is_some());
    assert!(speaker_segments.is_some());
    
    // Verify JSON field
    if let Some(segments) = speaker_segments {
        assert!(segments.is_array());
        let segments_array = segments.as_array().unwrap();
        assert_eq!(segments_array.len(), 1);
        assert_eq!(segments_array[0]["speaker"], 0);
    }
    
    Ok(())
}

/// Test pagination and complex queries
#[tokio::test]
async fn test_pagination_and_queries() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create test table
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_calls (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            system_id VARCHAR(50) NOT NULL,
            talkgroup_id INTEGER,
            transcription_status VARCHAR(20) DEFAULT 'none'
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Insert test data
    let systems = ["police", "fire", "ems"];
    let mut inserted_ids = Vec::new();
    
    for (i, system) in systems.iter().enumerate() {
        for j in 0..10 {
            let talkgroup_id = (i as i32 + 1) * 1000 + j;
            let status = match j % 3 {
                0 => "none",
                1 => "completed",
                _ => "failed",
            };
            
            let id = Uuid::new_v4();
            inserted_ids.push((id, system.to_string(), talkgroup_id, status.to_string()));
            
            sqlx::query("INSERT INTO test_calls (id, system_id, talkgroup_id, transcription_status) VALUES ($1, $2, $3, $4)")
                .bind(id)
                .bind(system)
                .bind(talkgroup_id)
                .bind(status)
                .execute(db.pool())
                .await?;
        }
    }
    
    // Test basic pagination
    let page_size = 5;
    let offset = 0;
    
    let rows = sqlx::query("SELECT * FROM test_calls ORDER BY created_at LIMIT $1 OFFSET $2")
        .bind(page_size)
        .bind(offset)
        .fetch_all(db.pool())
        .await?;
    
    assert_eq!(rows.len(), page_size as usize);
    
    // Test filtering
    let police_rows = sqlx::query("SELECT * FROM test_calls WHERE system_id = 'police'")
        .fetch_all(db.pool())
        .await?;
    
    assert_eq!(police_rows.len(), 10);
    
    // Test complex filtering
    let completed_police = sqlx::query(
        "SELECT * FROM test_calls WHERE system_id = 'police' AND transcription_status = 'completed'"
    )
    .fetch_all(db.pool())
    .await?;
    
    // Should be about 3-4 records (every 3rd record is 'completed')
    assert!(!completed_police.is_empty());
    
    // Test aggregation
    let count_row = sqlx::query("SELECT COUNT(*) as total FROM test_calls")
        .fetch_one(db.pool())
        .await?;
    
    let total: i64 = count_row.get("total");
    assert_eq!(total, 30); // 3 systems * 10 calls each
    
    // Test group by
    let system_counts = sqlx::query("SELECT system_id, COUNT(*) as count FROM test_calls GROUP BY system_id ORDER BY system_id")
        .fetch_all(db.pool())
        .await?;
    
    assert_eq!(system_counts.len(), 3);
    for row in system_counts {
        let count: i64 = row.get("count");
        assert_eq!(count, 10);
    }
    
    Ok(())
}

/// Test concurrent database operations
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create test table
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_concurrent (
            id SERIAL PRIMARY KEY,
            thread_id INTEGER NOT NULL,
            operation_id INTEGER NOT NULL,
            timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Spawn multiple concurrent tasks
    let task_count = 20;
    let operations_per_task = 10;
    let mut handles = Vec::new();
    
    for thread_id in 0..task_count {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            for operation_id in 0..operations_per_task {
                let result = sqlx::query("INSERT INTO test_concurrent (thread_id, operation_id) VALUES ($1, $2) RETURNING id")
                    .bind(thread_id)
                    .bind(operation_id)
                    .fetch_one(db_clone.pool())
                    .await;
                
                match result {
                    Ok(row) => {
                        let id: i32 = row.get("id");
                        results.push(id);
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok::<Vec<i32>, sqlx::Error>(results)
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut all_ids = Vec::new();
    for handle in handles {
        let results = handle.await??;
        all_ids.extend(results);
    }
    
    // Verify all operations completed
    assert_eq!(all_ids.len(), (task_count * operations_per_task) as usize);
    
    // Verify database consistency
    let count_row = sqlx::query("SELECT COUNT(*) as count FROM test_concurrent")
        .fetch_one(db.pool())
        .await?;
    
    let count: i64 = count_row.get("count");
    assert_eq!(count, (task_count * operations_per_task) as i64);
    
    // Verify no duplicate IDs
    all_ids.sort();
    let mut unique_ids = all_ids.clone();
    unique_ids.dedup();
    assert_eq!(all_ids.len(), unique_ids.len(), "Found duplicate IDs");
    
    Ok(())
}

/// Test database performance characteristics
#[tokio::test]
async fn test_database_performance() -> Result<()> {
    init_test_logging();
    
    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    
    // Create test table with index
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_performance (
            id SERIAL PRIMARY KEY,
            system_id VARCHAR(50) NOT NULL,
            talkgroup_id INTEGER NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            data TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_system_talkgroup ON test_performance(system_id, talkgroup_id);
        CREATE INDEX IF NOT EXISTS idx_created_at ON test_performance(created_at);
    "#;
    
    sqlx::query(create_table_sql)
        .execute(db.pool())
        .await?;
    
    // Insert larger dataset
    let timer = crate::common::helpers::PerformanceTimer::start("batch_insert");
    
    let batch_size = 1000;
    for batch in 0..5 {
        let mut query_builder = sqlx::QueryBuilder::new("INSERT INTO test_performance (system_id, talkgroup_id, data) ");
        query_builder.push_values(0..batch_size, |mut b, i| {
            let system_id = format!("system_{}", (batch * batch_size + i) % 10);
            let talkgroup_id = ((batch * batch_size + i) % 100) as i32;
            let data = format!("test_data_batch_{batch}_item_{i}");
            
            b.push_bind(system_id)
                .push_bind(talkgroup_id)
                .push_bind(data);
        });
        
        let query = query_builder.build();
        query.execute(db.pool()).await?;
    }
    
    let insert_duration = timer.stop();
    println!("Batch insert took: {insert_duration:?}");
    
    // Test query performance
    let timer = crate::common::helpers::PerformanceTimer::start("indexed_query");
    
    let rows = sqlx::query("SELECT * FROM test_performance WHERE system_id = $1 AND talkgroup_id = $2")
        .bind("system_5")
        .bind(25i32)
        .fetch_all(db.pool())
        .await?;
    
    let query_duration = timer.stop();
    println!("Indexed query took: {query_duration:?}");
    
    assert!(!rows.is_empty());
    
    // Test range query
    let timer = crate::common::helpers::PerformanceTimer::start("range_query");
    
    let now = chrono::Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    
    let recent_rows = sqlx::query("SELECT * FROM test_performance WHERE created_at > $1")
        .bind(one_hour_ago)
        .fetch_all(db.pool())
        .await?;
    
    let range_query_duration = timer.stop();
    println!("Range query took: {range_query_duration:?}");
    
    // All records should be recent
    assert_eq!(recent_rows.len(), 5000);
    
    // Ensure queries are reasonably fast (adjust thresholds as needed)
    assert!(insert_duration < tokio::time::Duration::from_secs(10), "Batch insert took too long");
    assert!(query_duration < tokio::time::Duration::from_millis(100), "Indexed query took too long");
    assert!(range_query_duration < tokio::time::Duration::from_secs(1), "Range query took too long");

    Ok(())
}

/// Negative test: Insert with duplicate primary key should fail
#[tokio::test]
async fn test_insert_with_duplicate_id_returns_error() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    let call_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // First insert should succeed
    let insert_sql = r#"
        INSERT INTO radio_calls (
            id, created_at, call_timestamp, system_id, system_label,
            talkgroup_id, transcription_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
    "#;

    sqlx::query(insert_sql)
        .bind(call_id)
        .bind(now)
        .bind(now)
        .bind("test_system")
        .bind("Test System")
        .bind(12345i32)
        .bind("none")
        .execute(db.pool())
        .await?;

    // Second insert with same ID should fail
    let result = sqlx::query(insert_sql)
        .bind(call_id) // Same ID - should fail
        .bind(now)
        .bind(now)
        .bind("test_system2")
        .bind("Test System 2")
        .bind(54321i32)
        .bind("none")
        .execute(db.pool())
        .await;

    assert!(result.is_err(), "Duplicate ID insert should fail");

    // Verify error is a database constraint violation
    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(
            error_msg.contains("duplicate") || error_msg.contains("unique") || error_msg.contains("constraint"),
            "Error should indicate constraint violation: {}",
            error_msg
        );
    }

    Ok(())
}

/// Negative test: Query with invalid UUID string should fail
#[tokio::test]
async fn test_query_with_invalid_uuid_returns_error() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    // Try to bind an invalid UUID string
    let result = sqlx::query("SELECT * FROM radio_calls WHERE id = $1")
        .bind("not-a-valid-uuid")
        .fetch_optional(db.pool())
        .await;

    assert!(result.is_err(), "Invalid UUID should cause error");

    Ok(())
}

/// Negative test: Connection pool exhaustion and recovery
#[tokio::test]
async fn test_connection_pool_exhaustion_recovery() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    // Get pool stats
    let pool_size = db.pool().size();
    println!("Pool size: {}", pool_size);

    // Pool should handle concurrent requests gracefully
    let mut handles = vec![];
    for i in 0..20 {
        let pool = db.pool().clone();
        let handle = tokio::spawn(async move {
            let result = sqlx::query("SELECT $1 as num")
                .bind(i)
                .fetch_one(&pool)
                .await;
            result.is_ok()
        });
        handles.push(handle);
    }

    // All requests should complete (may queue if pool is full)
    let results = futures::future::join_all(handles).await;
    let successful = results.iter().filter(|r| r.is_ok() && r.as_ref().unwrap()).count();

    assert_eq!(successful, 20, "All requests should eventually succeed despite pool limits");

    Ok(())
}

/// Negative test: Transaction rollback on error
#[tokio::test]
async fn test_transaction_rollback_on_error() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    let call_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Start transaction
    let mut tx = db.pool().begin().await?;

    // Insert a record
    sqlx::query(
        r#"
        INSERT INTO radio_calls (
            id, created_at, call_timestamp, system_id,
            talkgroup_id, transcription_status
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#
    )
    .bind(call_id)
    .bind(now)
    .bind(now)
    .bind("test_system")
    .bind(12345i32)
    .bind("pending")
    .execute(&mut *tx)
    .await?;

    // Explicitly rollback
    tx.rollback().await?;

    // Verify record was NOT inserted (transaction rolled back)
    let result = sqlx::query("SELECT * FROM radio_calls WHERE id = $1")
        .bind(call_id)
        .fetch_optional(db.pool())
        .await?;

    assert!(result.is_none(), "Record should not exist after rollback");

    Ok(())
}

/// Concurrent test: Multiple simultaneous inserts
#[tokio::test]
async fn test_concurrent_inserts_maintain_integrity() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    let now = chrono::Utc::now();

    // Spawn 10 concurrent insert tasks
    let mut handles = Vec::new();
    for i in 0..10 {
        let pool = db.pool().clone();
        let timestamp = now;

        let handle = tokio::spawn(async move {
            let call_id = Uuid::new_v4();
            let system_id = format!("concurrent_system_{i}");

            sqlx::query(
                r#"
                INSERT INTO radio_calls (
                    id, created_at, call_timestamp, system_id,
                    talkgroup_id, transcription_status
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(call_id)
            .bind(timestamp)
            .bind(timestamp)
            .bind(&system_id)
            .bind(1000 + i)
            .bind("pending")
            .execute(&pool)
            .await?;

            Ok::<_, sqlx::Error>(call_id)
        });

        handles.push(handle);
    }

    // Wait for all inserts to complete
    let mut inserted_ids = Vec::new();
    for handle in handles {
        let call_id = handle.await.expect("Task should complete")?;
        inserted_ids.push(call_id);
    }

    // Verify all 10 records were inserted
    assert_eq!(inserted_ids.len(), 10);

    // Verify all records are in the database
    for call_id in &inserted_ids {
        let result = sqlx::query("SELECT * FROM radio_calls WHERE id = $1")
            .bind(call_id)
            .fetch_one(db.pool())
            .await?;

        let id: Uuid = result.get("id");
        assert_eq!(&id, call_id);
    }

    // Verify no data corruption - count should be exactly 10
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM radio_calls")
        .fetch_one(db.pool())
        .await?;

    assert_eq!(count, 10, "Should have exactly 10 records");

    Ok(())
}

/// Concurrent test: Simultaneous reads while writing
#[tokio::test]
async fn test_concurrent_reads_while_writing() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    let now = chrono::Utc::now();

    // Insert initial record
    let initial_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO radio_calls (
            id, created_at, call_timestamp, system_id,
            talkgroup_id, transcription_status
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(initial_id)
    .bind(now)
    .bind(now)
    .bind("read_write_test")
    .bind(5000i32)
    .bind("pending")
    .execute(db.pool())
    .await?;

    // Spawn writer task that inserts records continuously
    let writer_pool = db.pool().clone();
    let writer_handle = tokio::spawn(async move {
        for i in 0..5 {
            let call_id = Uuid::new_v4();
            let timestamp = chrono::Utc::now();

            let result = sqlx::query(
                r#"
                INSERT INTO radio_calls (
                    id, created_at, call_timestamp, system_id,
                    talkgroup_id, transcription_status
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(call_id)
            .bind(timestamp)
            .bind(timestamp)
            .bind("writer_system")
            .bind(6000 + i)
            .bind("pending")
            .execute(&writer_pool)
            .await;

            if result.is_err() {
                return Err(sdrtrunk_core::context_error::ContextError::new(
                    "Writer task failed".to_string(),
                ));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        Ok::<_, sdrtrunk_core::context_error::ContextError>(())
    });

    // Spawn multiple reader tasks that query simultaneously
    let mut reader_handles = Vec::new();
    for _ in 0..5 {
        let reader_pool = db.pool().clone();

        let handle = tokio::spawn(async move {
            let mut read_count = 0;

            for _ in 0..10 {
                let result = sqlx::query("SELECT COUNT(*) as count FROM radio_calls")
                    .fetch_one(&reader_pool)
                    .await;

                if result.is_ok() {
                    read_count += 1;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            Ok::<_, sdrtrunk_core::context_error::ContextError>(read_count)
        });

        reader_handles.push(handle);
    }

    // Wait for writer to complete
    writer_handle
        .await
        .expect("Writer task should complete")?;

    // Wait for all readers to complete
    for handle in reader_handles {
        let read_count = handle.await.expect("Reader task should complete")?;
        assert!(
            read_count > 0,
            "Reader should have completed at least one read"
        );
    }

    // Verify final state
    let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM radio_calls")
        .fetch_one(db.pool())
        .await?;

    assert_eq!(
        final_count, 6,
        "Should have 1 initial + 5 writer records = 6 total"
    );

    Ok(())
}

/// Concurrent test: Connection pool exhaustion and recovery
#[tokio::test]
async fn test_connection_pool_exhaustion_recovery() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();

    // Spawn many concurrent queries (more than pool size)
    let mut handles = Vec::new();
    for i in 0..20 {
        let pool = db.pool().clone();

        let handle = tokio::spawn(async move {
            // Simple query that should succeed even under load
            let result: i32 = sqlx::query_scalar("SELECT $1::int")
                .bind(i)
                .fetch_one(&pool)
                .await?;

            Ok::<_, sqlx::Error>(result)
        });

        handles.push(handle);
    }

    // Wait for all queries to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should complete")?;
        results.push(result);
    }

    // Verify all queries completed successfully
    assert_eq!(results.len(), 20);

    // Verify results are correct (0..20)
    for (i, result) in results.iter().enumerate() {
        assert_eq!(*result, i as i32);
    }

    Ok(())
}

/// Concurrent test: No deadlocks on concurrent inserts to same table
#[tokio::test]
async fn test_concurrent_inserts_no_deadlock() -> Result<()> {
    init_test_logging();

    let test_db = TestDatabase::new().await?;
    let db = test_db.database();
    let now = chrono::Utc::now();

    // Use a timeout to detect deadlocks
    let timeout = tokio::time::Duration::from_secs(10);

    let test_future = async {
        // Spawn many concurrent insert tasks to the same table
        let mut handles = Vec::new();
        for i in 0..15 {
            let pool = db.pool().clone();
            let timestamp = now;

            let handle = tokio::spawn(async move {
                let call_id = Uuid::new_v4();

                sqlx::query(
                    r#"
                    INSERT INTO radio_calls (
                        id, created_at, call_timestamp, system_id,
                        talkgroup_id, transcription_status
                    ) VALUES ($1, $2, $3, $4, $5, $6)
                    "#,
                )
                .bind(call_id)
                .bind(timestamp)
                .bind(timestamp)
                .bind("deadlock_test")
                .bind(7000 + i)
                .bind("pending")
                .execute(&pool)
                .await?;

                Ok::<_, sqlx::Error>(call_id)
            });

            handles.push(handle);
        }

        // Wait for all inserts to complete
        for handle in handles {
            handle.await.expect("Task should complete")?;
        }

        Ok::<_, sdrtrunk_core::context_error::ContextError>(())
    };

    // Run with timeout - if it times out, there's likely a deadlock
    match tokio::time::timeout(timeout, test_future).await {
        Ok(result) => result?,
        Err(_) => panic!("Test timed out - possible deadlock detected"),
    }

    // Verify all records were inserted
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM radio_calls")
        .fetch_one(db.pool())
        .await?;

    assert_eq!(count, 15, "All 15 records should be inserted");

    Ok(())
}