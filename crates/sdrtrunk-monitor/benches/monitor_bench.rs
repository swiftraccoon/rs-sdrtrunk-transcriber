//! Benchmarks for the file monitoring service

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use sdrtrunk_monitor::{FileQueue, config::*};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Benchmark file pattern matching
fn bench_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    let test_paths = vec![
        "/tmp/test.mp3",
        "/tmp/system_TG123_20240101_120000.mp3",
        "/tmp/very_long_filename_with_lots_of_text.mp3",
        "/tmp/test.wav",
        "/tmp/test.txt",
        "/tmp/subdir/nested/file.mp3",
    ];

    let patterns = vec!["*.mp3".to_string()];
    let extensions = vec!["mp3".to_string()];

    for path_str in &test_paths {
        let path = PathBuf::from(path_str);
        group.bench_with_input(
            BenchmarkId::new(
                "matches_patterns",
                path.file_name().unwrap().to_string_lossy(),
            ),
            &path,
            |b, path| {
                b.iter(|| {
                    sdrtrunk_monitor::monitor::FileMonitor::matches_patterns(
                        black_box(path),
                        black_box(&patterns),
                        black_box(&extensions),
                    )
                });
            },
        );
    }

    group.finish();
}

/// Benchmark queue operations
fn bench_queue_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("queue_operations");

    // Benchmark enqueueing files
    group.bench_function("enqueue_single", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.mp3");
            tokio::fs::write(&test_file, b"test content").await.unwrap();

            let queue = FileQueue::new(1000, None, false, false);

            black_box(queue.enqueue(test_file).await.unwrap());
        });
    });

    // Benchmark dequeuing files
    group.bench_function("dequeue_single", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.mp3");
            tokio::fs::write(&test_file, b"test content").await.unwrap();

            let queue = FileQueue::new(1000, None, false, false);
            queue.enqueue(test_file).await.unwrap();

            black_box(queue.dequeue().await.unwrap());
        });
    });

    // Benchmark queue with priority
    group.bench_function("priority_queue_operations", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let queue = FileQueue::new(1000, None, true, true);

            // Enqueue multiple files
            for i in 0..10 {
                let test_file = temp_dir.path().join(format!("test_{i}.mp3"));
                tokio::fs::write(&test_file, format!("test content {i}"))
                    .await
                    .unwrap();
                queue.enqueue(test_file).await.unwrap();
            }

            // Dequeue all files
            while queue.dequeue().await.is_some() {
                // Process files
            }
        });
    });

    group.finish();
}

/// Benchmark configuration loading
fn bench_config_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_loading");

    group.bench_function("default_config", |b| {
        b.iter(|| {
            black_box(MonitorConfig::default());
        });
    });

    // Benchmark TOML serialization/deserialization
    group.bench_function("toml_serialize", |b| {
        let config = MonitorConfig::default();
        b.iter(|| {
            black_box(toml::to_string(&config).unwrap());
        });
    });

    group.bench_function("toml_deserialize", |b| {
        let config = MonitorConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        b.iter(|| {
            black_box(toml::from_str::<MonitorConfig>(&toml_str).unwrap());
        });
    });

    group.finish();
}

/// Benchmark file metadata extraction
fn bench_metadata_extraction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("metadata_extraction");

    // Create test files of different sizes
    let file_sizes = vec![1024, 10_240, 102_400, 1_024_000]; // 1KB, 10KB, 100KB, 1MB

    for size in file_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("extract_metadata", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = TempDir::new().unwrap();
                    let test_file = temp_dir.path().join("test.mp3");
                    let content = vec![0u8; size];
                    tokio::fs::write(&test_file, content).await.unwrap();

                    // Simulate metadata extraction
                    let metadata = tokio::fs::metadata(&test_file).await.unwrap();
                    black_box((
                        metadata.len(),
                        metadata.modified().unwrap(),
                        test_file.extension(),
                    ));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent queue operations
fn bench_concurrent_queue(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_queue");

    // Benchmark concurrent enqueuing
    group.bench_function("concurrent_enqueue", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let queue = std::sync::Arc::new(FileQueue::new(10000, None, false, false));

            let mut handles = Vec::new();

            // Spawn multiple tasks to enqueue files concurrently
            for i in 0..100 {
                let queue = queue.clone();
                let temp_dir = temp_dir.path().to_path_buf();

                let handle = tokio::spawn(async move {
                    let test_file = temp_dir.join(format!("test_{i}.mp3"));
                    tokio::fs::write(&test_file, format!("content {i}"))
                        .await
                        .unwrap();
                    queue.enqueue(test_file).await.unwrap()
                });

                handles.push(handle);
            }

            // Wait for all tasks to complete
            for handle in handles {
                black_box(handle.await.unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark system info extraction from filenames
fn bench_filename_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("filename_parsing");

    let test_filenames = vec![
        "System123_TG456_20240101_120000.mp3",
        "VeryLongSystemName_TG999999_20241231_235959.mp3",
        "SimpleFile.mp3",
        "Complex_System_Name_With_Underscores_TG123_20240615_143022.mp3",
        "InvalidFormat.mp3",
    ];

    // Create a mock processor for testing
    let temp_dir = TempDir::new().unwrap();

    for filename in &test_filenames {
        group.bench_with_input(
            BenchmarkId::new("parse_filename", filename),
            filename,
            |b, filename| {
                b.iter(|| {
                    // Simulate the filename parsing logic
                    let parts: Vec<&str> = filename.split('_').collect();
                    black_box(if parts.len() >= 2 {
                        let system_name = parts[0];
                        let talkgroup = parts
                            .iter()
                            .find(|part| {
                                part.starts_with("TG")
                                    && part[2..].chars().all(|c| c.is_ascii_digit())
                            })
                            .and_then(|tg| tg[2..].parse::<u32>().ok());

                        Some((system_name, talkgroup))
                    } else {
                        None
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pattern_matching,
    bench_queue_operations,
    bench_config_loading,
    bench_metadata_extraction,
    bench_concurrent_queue,
    bench_filename_parsing
);

criterion_main!(benches);
