//! Benchmarks for sdrtrunk-core functionality

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use sdrtrunk_core::{types::*, utils::*, Config};
use std::path::PathBuf;

/// Benchmark filename parsing performance
fn bench_filename_parsing(c: &mut Criterion) {
    let filenames = vec![
        "20240315_142530_Metro_TG52197_FROM_1234567.mp3",
        "20240101_000000_System1_TG1_FROM_1.mp3", 
        "20231225_235959_Fire_Department_TG99999_FROM_9999999.wav",
        "20240630_180000_EMS_Services_Dispatch_TG12345_FROM_678901.flac",
        "20240229_120000_Police_North_District_TG54321_FROM_987654.mp3",
    ];
    
    let mut group = c.benchmark_group("filename_parsing");
    
    for filename in &filenames {
        group.bench_with_input(
            BenchmarkId::new("parse_sdrtrunk_filename", filename),
            filename,
            |b, filename| {
                b.iter(|| parse_sdrtrunk_filename(filename))
            },
        );
    }
    
    // Benchmark batch parsing
    group.throughput(Throughput::Elements(filenames.len() as u64));
    group.bench_function("parse_batch", |b| {
        b.iter(|| {
            for filename in &filenames {
                let _ = parse_sdrtrunk_filename(filename);
            }
        })
    });
    
    group.finish();
}

/// Benchmark file extension validation
fn bench_file_extension_validation(c: &mut Criterion) {
    let allowed_extensions = vec![
        "mp3".to_string(),
        "wav".to_string(), 
        "flac".to_string(),
        "m4a".to_string(),
        "aac".to_string(),
    ];
    
    let test_files = vec![
        "test.mp3",
        "audio.wav",
        "music.flac",
        "voice.m4a",
        "song.aac",
        "document.txt",
        "image.png",
        "data.json",
        "file_without_extension",
        "path/to/nested/file.mp3",
    ];
    
    let mut group = c.benchmark_group("file_extension_validation");
    
    group.throughput(Throughput::Elements(test_files.len() as u64));
    group.bench_function("validate_extensions", |b| {
        b.iter(|| {
            for filename in &test_files {
                let _ = validate_file_extension(filename, &allowed_extensions);
            }
        })
    });
    
    group.finish();
}

/// Benchmark storage filename generation
fn bench_storage_filename_generation(c: &mut Criterion) {
    let test_files = vec![
        "original.mp3",
        "test_file.wav",
        "long_filename_with_many_parts.flac",
        "file",
        "a.b",
    ];
    
    let mut group = c.benchmark_group("storage_filename_generation");
    
    for filename in &test_files {
        group.bench_with_input(
            BenchmarkId::new("generate_storage_filename", filename),
            filename,
            |b, filename| {
                b.iter(|| generate_storage_filename(filename))
            },
        );
    }
    
    group.finish();
}

/// Benchmark date path creation
fn bench_date_path_creation(c: &mut Criterion) {
    let base = PathBuf::from("/data/storage");
    let dates = vec![
        chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 30, 45).unwrap(),
        chrono::Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap(),
        chrono::Utc.with_ymd_and_hms(2023, 2, 28, 8, 15, 22).unwrap(),
        chrono::Utc.with_ymd_and_hms(2025, 7, 4, 16, 45, 10).unwrap(),
    ];
    
    let mut group = c.benchmark_group("date_path_creation");
    
    group.throughput(Throughput::Elements(dates.len() as u64));
    group.bench_function("create_date_paths", |b| {
        b.iter(|| {
            for date in &dates {
                let _ = create_date_path(&base, date);
            }
        })
    });
    
    group.finish();
}

/// Benchmark system ID validation
fn bench_system_id_validation(c: &mut Criterion) {
    let system_ids = vec![
        "valid_system",
        "metro-police",
        "fire_dept_123",
        "a".repeat(50), // At limit
        "a".repeat(51), // Over limit
        "",             // Empty
        "system with spaces",
        "system@invalid",
        "system/path",
        "normalid",
    ];
    
    let mut group = c.benchmark_group("system_id_validation");
    
    group.throughput(Throughput::Elements(system_ids.len() as u64));
    group.bench_function("validate_system_ids", |b| {
        b.iter(|| {
            for system_id in &system_ids {
                let _ = validate_system_id(system_id);
            }
        })
    });
    
    group.finish();
}

/// Benchmark filename sanitization
fn bench_filename_sanitization(c: &mut Criterion) {
    let filenames = vec![
        "normal_filename.mp3",
        "file with spaces.mp3",
        "file@#$%^&*().mp3",
        "file/path\\name.mp3",
        "___file___.mp3",
        "unicode_café_файл_🎵.mp3",
        "very/long\\path@with*many|problematic:characters?.mp3",
        "",
        "...",
        "a".repeat(255),
    ];
    
    let mut group = c.benchmark_group("filename_sanitization");
    
    group.throughput(Throughput::Elements(filenames.len() as u64));
    group.bench_function("sanitize_filenames", |b| {
        b.iter(|| {
            for filename in &filenames {
                let _ = sanitize_filename(filename);
            }
        })
    });
    
    group.finish();
}

/// Benchmark duration formatting and parsing
fn bench_duration_operations(c: &mut Criterion) {
    let durations = vec![
        0.0, 30.0, 90.0, 3661.0, 86400.0, // 0s, 30s, 1m30s, 1h1m1s, 24h
    ];
    
    let duration_strings = vec![
        "00:00", "00:30", "01:30", "01:01:01", "24:00:00",
    ];
    
    let mut group = c.benchmark_group("duration_operations");
    
    group.bench_function("format_durations", |b| {
        b.iter(|| {
            for &duration in &durations {
                let _ = format_duration(duration);
            }
        })
    });
    
    group.bench_function("parse_durations", |b| {
        b.iter(|| {
            for duration_str in &duration_strings {
                let _ = parse_duration(duration_str);
            }
        })
    });
    
    group.finish();
}

/// Benchmark frequency formatting
fn bench_frequency_formatting(c: &mut Criterion) {
    let frequencies = vec![
        500,         // Hz
        1500,        // kHz
        154000000,   // MHz
        854000000,   // MHz
        2400000000,  // GHz
        999999999999, // Large MHz
    ];
    
    let mut group = c.benchmark_group("frequency_formatting");
    
    group.throughput(Throughput::Elements(frequencies.len() as u64));
    group.bench_function("format_frequencies", |b| {
        b.iter(|| {
            for &frequency in &frequencies {
                let _ = format_frequency(frequency);
            }
        })
    });
    
    group.finish();
}

/// Benchmark checksum calculation
fn bench_checksum_calculation(c: &mut Criterion) {
    let data_sizes = vec![
        100,      // 100 bytes
        1024,     // 1KB
        10240,    // 10KB
        102400,   // 100KB
        1048576,  // 1MB
    ];
    
    let mut group = c.benchmark_group("checksum_calculation");
    
    for &size in &data_sizes {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("calculate_checksum", size),
            &data,
            |b, data| {
                b.iter(|| calculate_file_checksum(data))
            },
        );
    }
    
    group.finish();
}

/// Benchmark configuration serialization/deserialization
fn bench_config_serialization(c: &mut Criterion) {
    let configs = vec![
        Config::default(),
        {
            let mut config = Config::default();
            config.server.workers = 16;
            config.database.max_connections = 100;
            config.api.enable_cors = true;
            config.api.cors_origins = vec!["*".to_string()];
            config.storage.allowed_extensions = vec![
                "mp3".to_string(), "wav".to_string(), "flac".to_string(),
                "m4a".to_string(), "aac".to_string(), "ogg".to_string(),
            ];
            config
        },
    ];
    
    let mut group = c.benchmark_group("config_serialization");
    
    for (i, config) in configs.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("serialize_config", i),
            config,
            |b, config| {
                b.iter(|| serde_json::to_string(config))
            },
        );
        
        let serialized = serde_json::to_string(config).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize_config", i),
            &serialized,
            |b, serialized| {
                b.iter(|| {
                    let _: Config = serde_json::from_str(serialized).unwrap();
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark RadioCall serialization/deserialization
fn bench_radio_call_serialization(c: &mut Criterion) {
    let radio_calls = vec![
        RadioCall::default(),
        {
            let mut call = RadioCall::default();
            call.id = Some(uuid::Uuid::new_v4());
            call.system_id = "metro_police".to_string();
            call.system_label = Some("Metro Police Department".to_string());
            call.frequency = Some(854000000);
            call.talkgroup_id = Some(52197);
            call.talkgroup_label = Some("Police Dispatch".to_string());
            call.source_radio_id = Some(1234567);
            call.transcription_status = TranscriptionStatus::Completed;
            call.transcription_text = Some("Unit 23 responding to traffic stop".to_string());
            call.transcription_confidence = Some(0.95);
            call.patches = Some(serde_json::json!({
                "patch_groups": [{"id": 1, "name": "District 1"}]
            }));
            call
        },
    ];
    
    let mut group = c.benchmark_group("radio_call_serialization");
    
    for (i, call) in radio_calls.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("serialize_radio_call", i),
            call,
            |b, call| {
                b.iter(|| serde_json::to_string(call))
            },
        );
        
        let serialized = serde_json::to_string(call).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize_radio_call", i),
            &serialized,
            |b, serialized| {
                b.iter(|| {
                    let _: RadioCall = serde_json::from_str(serialized).unwrap();
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark large-scale operations
fn bench_large_scale_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_scale_operations");
    
    // Generate large number of filenames
    let large_filename_batch: Vec<String> = (0..10000)
        .map(|i| format!("20240315_{:06}_System{}_TG{}_FROM_{}.mp3", 
                        i, i % 10, 10000 + i, 1000000 + i))
        .collect();
    
    group.throughput(Throughput::Elements(large_filename_batch.len() as u64));
    group.bench_function("parse_10000_filenames", |b| {
        b.iter(|| {
            let mut success_count = 0;
            for filename in &large_filename_batch {
                if parse_sdrtrunk_filename(filename).is_ok() {
                    success_count += 1;
                }
            }
            success_count
        })
    });
    
    // Generate large number of storage filenames
    group.bench_function("generate_10000_storage_filenames", |b| {
        b.iter(|| {
            let mut filenames = Vec::with_capacity(10000);
            for i in 0..10000 {
                filenames.push(generate_storage_filename(&format!("file_{i}.mp3")));
            }
            filenames
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_filename_parsing,
    bench_file_extension_validation,
    bench_storage_filename_generation,
    bench_date_path_creation,
    bench_system_id_validation,
    bench_filename_sanitization,
    bench_duration_operations,
    bench_frequency_formatting,
    bench_checksum_calculation,
    bench_config_serialization,
    bench_radio_call_serialization,
    bench_large_scale_operations,
);

criterion_main!(benches);