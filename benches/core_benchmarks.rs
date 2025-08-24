//! Comprehensive benchmarks for sdrtrunk-core functionality

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sdrtrunk_core::utils::*;

/// Benchmark filename parsing with realistic data
fn bench_filename_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("filename_parsing");

    // Realistic SDRTrunk filenames
    let filenames = vec![
        "20240315_142530_Metro_TG52197_FROM_1234567.mp3",
        "20240101_000000_System1_TG1_FROM_1.mp3",
        "20231225_235959_Fire_Department_TG99999_FROM_9999999.wav",
        "20240630_180000_EMS_Services_Dispatch_TG12345_FROM_678901.flac",
        "20240229_120000_Police_North_District_TG54321_FROM_987654.mp3",
        // Edge cases
        "invalid_filename.mp3",
        "20240315.mp3",
        "",
    ];

    // Benchmark individual parsing
    for filename in &filenames {
        if !filename.is_empty() {
            group.bench_with_input(
                BenchmarkId::new("parse", filename),
                filename,
                |b, filename| b.iter(|| parse_sdrtrunk_filename(filename)),
            );
        }
    }

    // Benchmark batch parsing (realistic workload)
    group.throughput(Throughput::Elements(filenames.len() as u64));
    group.bench_function("parse_batch", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(filenames.len());
            for filename in &filenames {
                results.push(parse_sdrtrunk_filename(filename));
            }
            results
        })
    });

    group.finish();
}

/// Benchmark file validation operations
fn bench_file_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_validation");

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
        "very/long/path/with/many/directories/audio.mp3",
        "../../../etc/passwd", // Security test
    ];

    // Benchmark extension validation
    group.throughput(Throughput::Elements(test_files.len() as u64));
    group.bench_function("validate_extensions", |b| {
        b.iter(|| {
            let mut valid_count = 0;
            for filename in &test_files {
                if validate_file_extension(filename, &allowed_extensions) {
                    valid_count += 1;
                }
            }
            valid_count
        })
    });

    // Benchmark system ID validation
    let at_limit = "a".repeat(50);
    let over_limit = "a".repeat(51);
    let system_ids: Vec<&str> = vec![
        "valid_system",
        "metro-police",
        "fire_dept_123",
        &at_limit,   // At limit
        &over_limit, // Over limit
        "",          // Empty
        "system with spaces",
        "system@invalid",
        "system/path",
        "../../../etc/passwd", // Security test
    ];

    group.bench_function("validate_system_ids", |b| {
        b.iter(|| {
            let mut valid_count = 0;
            for system_id in &system_ids {
                if validate_system_id(system_id) {
                    valid_count += 1;
                }
            }
            valid_count
        })
    });

    group.finish();
}

/// Benchmark file operations
fn bench_file_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_operations");

    // Benchmark storage filename generation with various inputs
    let test_files = vec![
        "original.mp3",
        "test_file.wav",
        "long_filename_with_many_parts_and_extensions.flac",
        "file",
        "a.b.c.d.e.mp3",
        "unicode_café_文件_🎵.mp3",
    ];

    group.bench_function("generate_storage_filenames", |b| {
        b.iter(|| {
            let mut filenames = Vec::with_capacity(test_files.len());
            for file in &test_files {
                filenames.push(generate_storage_filename(file));
            }
            filenames
        })
    });

    // Benchmark filename sanitization
    let dirty_filenames = vec![
        "normal_filename.mp3",
        "file with spaces.mp3",
        "file@#$%^&*().mp3",
        "file/path\\name.mp3",
        "___file___.mp3",
        "unicode_café_файл_🎵.mp3",
        "../../../etc/passwd.mp3",
        "very/long\\path@with*many|problematic:characters?.mp3",
    ];

    group.bench_function("sanitize_filenames", |b| {
        b.iter(|| {
            let mut sanitized = Vec::with_capacity(dirty_filenames.len());
            for filename in &dirty_filenames {
                sanitized.push(sanitize_filename(filename));
            }
            sanitized
        })
    });

    group.finish();
}

/// Benchmark checksum calculation with realistic file sizes
fn bench_checksum_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("checksum");

    // Realistic audio file sizes
    let data_sizes = vec![
        1024 * 10,       // 10KB - short clip
        1024 * 100,      // 100KB - typical short recording
        1024 * 500,      // 500KB - medium recording
        1024 * 1024,     // 1MB - longer recording
        1024 * 1024 * 5, // 5MB - long recording
    ];

    for &size in &data_sizes {
        // Create realistic data (not just zeros)
        let mut data = vec![0u8; size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("calculate", size / 1024),
            &data,
            |b, data| b.iter(|| calculate_file_checksum(data)),
        );
    }

    group.finish();
}

/// Benchmark formatting operations
fn bench_formatting_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatting");

    // Benchmark duration formatting
    let durations = vec![
        0.0,      // 0 seconds
        30.5,     // 30.5 seconds
        90.0,     // 1 minute 30 seconds
        3661.0,   // 1 hour 1 minute 1 second
        86400.0,  // 24 hours
        999999.0, // Large duration
    ];

    group.bench_function("format_durations", |b| {
        b.iter(|| {
            let mut formatted = Vec::with_capacity(durations.len());
            for &duration in &durations {
                formatted.push(format_duration(duration));
            }
            formatted
        })
    });

    // Benchmark duration parsing
    let duration_strings = vec![
        "00:00",
        "00:30",
        "01:30",
        "01:01:01",
        "24:00:00",
        "999:59:59",
        "invalid",
    ];

    group.bench_function("parse_durations", |b| {
        b.iter(|| {
            let mut parsed = Vec::with_capacity(duration_strings.len());
            for duration_str in &duration_strings {
                parsed.push(parse_duration(duration_str));
            }
            parsed
        })
    });

    // Benchmark frequency formatting
    let frequencies = vec![
        500,           // 500 Hz
        1500,          // 1.5 kHz
        154_000_000,   // 154 MHz (typical public safety)
        854_000_000,   // 854 MHz (typical public safety)
        2_400_000_000, // 2.4 GHz
        -1,            // Invalid
    ];

    group.bench_function("format_frequencies", |b| {
        b.iter(|| {
            let mut formatted = Vec::with_capacity(frequencies.len());
            for &frequency in &frequencies {
                formatted.push(format_frequency(frequency));
            }
            formatted
        })
    });

    group.finish();
}

/// Benchmark large-scale realistic operations
fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");

    // Generate realistic batch of files to process
    let mut filenames = Vec::new();
    for hour in 0..24 {
        for minute in 0..60 {
            for system in ["Metro", "Fire", "EMS", "Police"] {
                let filename = format!(
                    "202403{:02}_{:02}{:02}00_{}_TG{}_FROM_{}.mp3",
                    15,
                    hour,
                    minute,
                    system,
                    10000 + hour * 100 + minute,
                    1000000 + hour * 1000 + minute
                );
                filenames.push(filename);
            }
        }
    }

    // Benchmark processing a day's worth of files
    group.throughput(Throughput::Elements(filenames.len() as u64));
    group.bench_function("process_daily_files", |b| {
        b.iter(|| {
            let mut valid_files = 0;
            let mut total_size = 0u64;
            let allowed_ext = vec!["mp3".to_string()];

            for filename in &filenames {
                // Parse filename
                if let Ok(_data) = parse_sdrtrunk_filename(filename) {
                    // Validate extension
                    if validate_file_extension(filename, &allowed_ext) {
                        valid_files += 1;
                        // Generate storage name
                        let _storage = generate_storage_filename(filename);
                        // Simulate size calculation
                        total_size += filename.len() as u64 * 1000; // Fake size
                    }
                }
            }
            (valid_files, total_size)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_filename_parsing,
    bench_file_validation,
    bench_file_operations,
    bench_checksum_calculation,
    bench_formatting_operations,
    bench_realistic_workload
);

criterion_main!(benches);
