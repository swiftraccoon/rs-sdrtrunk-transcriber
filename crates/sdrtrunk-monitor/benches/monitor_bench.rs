//! Comprehensive benchmarks for the sdrtrunk-monitor functionality
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::no_effect_underscore_binding)]

use chrono::Utc;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use uuid::Uuid;

// Mock structures for benchmarking queue operations
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct QueuedFile {
    id: Uuid,
    path: PathBuf,
    size: u64,
    priority: i32,
    retry_count: u32,
}

impl Ord for QueuedFile {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then by size
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| self.size.cmp(&other.size))
    }
}

impl PartialOrd for QueuedFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QueuedFile {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for QueuedFile {}

/// Benchmark file path validation and filtering
fn bench_path_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_operations");

    // Realistic SDRTrunk audio file paths
    let test_paths = vec![
        PathBuf::from("/recordings/2024/03/15/System1_TG12345_20240315_142530.mp3"),
        PathBuf::from("/recordings/2024/03/15/Metro_TG52197_20240315_142530.wav"),
        PathBuf::from("/recordings/2024/03/15/Fire_TG99999_20240315_142530.flac"),
        PathBuf::from("/recordings/temp/processing_12345.mp3"),
        PathBuf::from("/recordings/archive/old_recording.mp3"),
        PathBuf::from("/recordings/2024/03/15/invalid.txt"),
        PathBuf::from("/recordings/2024/03/15/document.pdf"),
        PathBuf::from("/recordings/2024/03/15/.hidden_file.mp3"),
    ];

    let valid_extensions = ["mp3", "wav", "flac", "m4a"];

    // Benchmark extension checking
    group.throughput(Throughput::Elements(test_paths.len() as u64));
    group.bench_function("check_extensions", |b| {
        b.iter(|| {
            let mut valid_count = 0;
            for path in &test_paths {
                if let Some(ext) = path.extension()
                    && let Some(ext_str) = ext.to_str()
                    && valid_extensions.contains(&ext_str.to_lowercase().as_str())
                {
                    valid_count += 1;
                }
            }
            black_box(valid_count)
        })
    });

    // Benchmark pattern matching for SDRTrunk files
    let _patterns = [
        "System*_TG*_*.mp3",
        "Metro_TG*_*.mp3",
        "Fire_TG*_*.mp3",
        "Police_TG*_*.mp3",
        "*_TG*_202403*.mp3",
    ];

    group.bench_function("pattern_matching", |b| {
        b.iter(|| {
            let mut matches = 0;
            for path in &test_paths {
                if let Some(file_name) = path.file_name() {
                    let name_str = file_name.to_string_lossy();
                    // Simple pattern matching simulation
                    if name_str.contains("_TG") && name_str.contains("_202403") {
                        matches += 1;
                    }
                }
            }
            black_box(matches)
        })
    });

    // Benchmark metadata extraction from paths
    group.bench_function("extract_metadata", |b| {
        b.iter(|| {
            let mut metadata_items = Vec::with_capacity(test_paths.len());
            for path in &test_paths {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let parent = path.parent().map(|p| p.to_string_lossy().to_string());
                let is_hidden = stem.starts_with('.');

                metadata_items.push((stem.to_string(), extension.to_string(), parent, is_hidden));
            }
            black_box(metadata_items)
        })
    });

    group.finish();
}

/// Benchmark queue operations with realistic file processing
fn bench_queue_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_operations");

    // Generate realistic queue of files
    let queue_sizes = vec![10, 100, 500, 1000];

    for size in queue_sizes {
        // Create files with varying priorities and sizes
        let mut files = Vec::with_capacity(size);
        for i in 0..size {
            files.push(QueuedFile {
                id: Uuid::new_v4(),
                path: PathBuf::from(format!("/recordings/System_TG{}_file_{}.mp3", i % 100, i)),
                size: 100_000 + (i as u64 * 1000), // 100KB to several MB
                priority: i32::try_from(i % 5).unwrap_or(0),  // Priority 0-4
                retry_count: u32::try_from(i % 3).unwrap_or(0),
            });
        }

        // Benchmark priority queue operations
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("enqueue_dequeue", size),
            &files,
            |b, files| {
                b.iter(|| {
                    let mut queue: BinaryHeap<QueuedFile> = BinaryHeap::with_capacity(files.len());

                    // Enqueue all files
                    for file in files {
                        queue.push(file.clone());
                    }

                    // Dequeue all files
                    let mut processed = Vec::with_capacity(files.len());
                    while let Some(file) = queue.pop() {
                        processed.push(file);
                    }

                    black_box(processed)
                })
            },
        );

        // Benchmark concurrent queue access simulation
        group.bench_with_input(
            BenchmarkId::new("concurrent_access", size),
            &files,
            |b, files| {
                b.iter(|| {
                    let mut pending: BinaryHeap<QueuedFile> = BinaryHeap::new();
                    let mut processing = Vec::new();
                    let mut completed = Vec::new();

                    // Simulate processing workflow
                    for (i, file) in files.iter().enumerate() {
                        pending.push(file.clone());

                        // Every 10 files, process some
                        if i % 10 == 0
                            && !pending.is_empty()
                            && let Some(file) = pending.pop()
                        {
                            processing.push(file.clone());

                            // Simulate completion
                            if processing.len() > 5 {
                                completed.push(processing.remove(0));
                            }
                        }
                    }

                    black_box((pending.len(), processing.len(), completed.len()))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark file validation operations
fn bench_file_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_validation");

    // Test various file sizes for validation
    let file_sizes = vec![
        0,                 // Empty file
        1024,              // 1KB
        1024 * 100,        // 100KB
        1024 * 1024,       // 1MB
        1024 * 1024 * 10,  // 10MB
        1024 * 1024 * 100, // 100MB (max allowed)
        1024 * 1024 * 101, // Over limit
    ];

    group.bench_function("validate_file_sizes", |b| {
        b.iter(|| {
            let max_size = 100 * 1024 * 1024; // 100MB limit
            let mut valid_count = 0;
            let mut total_size = 0u64;

            for &size in &file_sizes {
                if size > 0 && size <= max_size {
                    valid_count += 1;
                    total_size += size;
                }
            }

            black_box((valid_count, total_size))
        })
    });

    // Benchmark filename sanitization
    let test_filenames = vec![
        "normal_file.mp3",
        "file with spaces.mp3",
        "file@#$%^&*().mp3",
        "../../../etc/passwd.mp3", // Path traversal attempt
        "file\x00null.mp3",        // Null byte
        "very_long_filename_that_exceeds_normal_limits_and_needs_to_be_truncated_somehow.mp3",
        "unicode_café_文件_🎵.mp3",
        ".hidden_file.mp3",
    ];

    group.bench_function("sanitize_filenames", |b| {
        b.iter(|| {
            let mut sanitized = Vec::with_capacity(test_filenames.len());

            for filename in &test_filenames {
                // Sanitize: remove path traversal, special chars, etc.
                let clean = filename
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                    .take(255) // Max filename length
                    .collect::<String>();

                sanitized.push(clean);
            }

            black_box(sanitized)
        })
    });

    // Benchmark checksum validation (simulate)
    group.bench_function("validate_checksums", |b| {
        let data_chunks = vec![
            vec![0u8; 1024],       // 1KB
            vec![1u8; 1024 * 10],  // 10KB
            vec![2u8; 1024 * 100], // 100KB
        ];

        b.iter(|| {
            let mut checksums = Vec::with_capacity(data_chunks.len());

            for chunk in &data_chunks {
                // Simulate checksum calculation
                let sum: u64 = chunk.iter().map(|&b| u64::from(b)).sum();
                checksums.push(sum);
            }

            black_box(checksums)
        })
    });

    group.finish();
}

/// Benchmark monitoring service operations
fn bench_monitoring_service(c: &mut Criterion) {
    let mut group = c.benchmark_group("monitoring_service");

    // Benchmark service status updates
    group.bench_function("status_updates", |b| {
        #[derive(Clone)]
        enum ServiceStatus {
            Starting,
            Running,
            Paused,
            Stopping,
            Stopped,
        }

        let statuses = vec![
            ServiceStatus::Starting,
            ServiceStatus::Running,
            ServiceStatus::Paused,
            ServiceStatus::Running,
            ServiceStatus::Stopping,
            ServiceStatus::Stopped,
        ];

        b.iter(|| {
            let mut current_status = ServiceStatus::Stopped;
            let mut transition_count = 0;

            for status in &statuses {
                current_status = status.clone();
                transition_count += 1;
            }

            black_box((current_status, transition_count))
        })
    });

    // Benchmark metrics collection
    group.bench_function("metrics_collection", |b| {
        b.iter(|| {
            let start_time = Utc::now();
            let mut metrics = vec![];

            for i in 0..100 {
                let metric = (
                    format!("metric_{i}"),
                    f64::from(i) * 1.5,
                    Utc::now().timestamp_millis() - start_time.timestamp_millis(),
                );
                metrics.push(metric);
            }

            // Calculate aggregates
            let sum: f64 = metrics.iter().map(|(_, v, _)| v).sum();
            let avg = sum / metrics.len() as f64;
            let max = metrics
                .iter()
                .map(|(_, v, _)| *v)
                .fold(0.0_f64, f64::max);

            black_box((metrics.len(), avg, max))
        })
    });

    // Benchmark batch file discovery
    group.bench_function("file_discovery", |b| {
        b.iter(|| {
            let mut discovered_files = Vec::new();
            let base_path = PathBuf::from("/recordings/2024/03/15");

            // Simulate discovering files in directory structure
            for hour in 0..24 {
                for minute in 0..60 {
                    if minute % 5 == 0 {
                        // Files every 5 minutes
                        let path = base_path.join(format!(
                            "System_TG12345_202403{:02}_{:02}{:02}00.mp3",
                            15, hour, minute
                        ));

                        // Check if should be processed
                        let should_process = (6..=22).contains(&hour); // Day hours only

                        if should_process {
                            discovered_files.push((path, 1024 * 100)); // 100KB files
                        }
                    }
                }
            }

            black_box(discovered_files.len())
        })
    });

    group.finish();
}

/// Benchmark realistic monitoring workload
fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(3));

    // Simulate processing a batch of incoming files
    group.bench_function("process_incoming_batch", |b| {
        b.iter(|| {
            let mut queue: BinaryHeap<QueuedFile> = BinaryHeap::new();
            let mut processed_count = 0;
            let mut total_size = 0u64;
            let mut errors = 0;

            // Generate batch of incoming files
            for i in 0..100 {
                let file = QueuedFile {
                    id: Uuid::new_v4(),
                    path: PathBuf::from(format!("/recordings/file_{i}.mp3")),
                    size: 100_000 + (u64::try_from(i).unwrap_or(0) * 5000),
                    priority: (100 - i) / 20, // Higher priority for newer files
                    retry_count: 0,
                };

                // Validate before queuing
                if file.size > 0 && file.size <= 100 * 1024 * 1024 {
                    queue.push(file.clone());
                    total_size += file.size;
                } else {
                    errors += 1;
                }
            }

            // Process queue
            let workers = 4;
            let mut active_workers = Vec::with_capacity(workers);

            while !queue.is_empty() || !active_workers.is_empty() {
                // Assign work to available workers
                while active_workers.len() < workers && !queue.is_empty() {
                    if let Some(file) = queue.pop() {
                        active_workers.push(file);
                    }
                }

                // Simulate processing completion
                if !active_workers.is_empty() {
                    processed_count += active_workers.len();
                    active_workers.clear();
                }
            }

            black_box((processed_count, total_size, errors))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_path_operations,
    bench_queue_operations,
    bench_file_validation,
    bench_monitoring_service,
    bench_realistic_workload
);

criterion_main!(benches);
