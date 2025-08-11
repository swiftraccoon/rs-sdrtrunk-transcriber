# Rust Development Standards

## Toolchain Requirements

### Minimum Rust Version

- **Rust**: 1.85.0 (latest stable)
- **Edition**: 2024
- **MSRV Policy**: Update MSRV quarterly, maintain compatibility for 6 months

### Required Components

```bash
rustup component add rustfmt clippy miri rust-src rust-analyzer llvm-tools-preview
```

### Essential Tools Installation

```bash
# Core development tools
cargo install cargo-audit --locked          # Security vulnerability scanning
cargo install cargo-deny --locked           # Dependency management and licensing
cargo install cargo-machete --locked        # Unused dependency detection
cargo install cargo-nextest --locked        # Fast test runner
cargo install cargo-llvm-cov --locked       # Code coverage analysis
cargo install cargo-expand --locked         # Macro expansion debugging
cargo install cargo-udeps --locked          # Unused dependency detection (nightly)
cargo install cargo-bloat --locked          # Binary size analysis
cargo install criterion-cli --locked        # Benchmark result management
cargo install flamegraph --locked           # Performance profiling

# Optional productivity tools  
cargo install cargo-watch --locked          # File watching for development
cargo install just --locked                 # Command runner (replaces make)
```

## Project Structure Standards

### Workspace Organization

```
rs-sdrtrunk-transcriber/
├── Cargo.toml                  # Workspace root with shared dependencies
├── rust-toolchain.toml         # Reproducible toolchain specification
├── .cargo/config.toml          # Cargo configuration and aliases
├── rustfmt.toml               # Code formatting rules
├── clippy.toml                # Linting configuration
├── deny.toml                  # Dependency management rules
├── justfile                   # Development automation
├── .editorconfig              # Editor consistency
├── .github/workflows/         # CI/CD pipelines
└── crates/                    # Individual crates
    ├── core/                  # Core abstractions and types
    ├── cli/                   # Command-line interface
    ├── server/                # Server implementation
    ├── client/                # Client libraries
    └── ffi/                   # Foreign function interfaces
```

### Module Organization Patterns

#### Crate Structure

```rust
src/
├── lib.rs                     # Public API and re-exports
├── error.rs                   # Error types and Result aliases
├── types.rs                   # Common types and type aliases
├── config.rs                  # Configuration structures
├── utils.rs                   # Utility functions
└── domain/                    # Domain-specific modules
    ├── mod.rs                 # Module declarations
    ├── audio.rs               # Audio processing
    ├── signal.rs              # Signal processing
    └── metadata.rs            # Metadata handling
```

#### Public API Design

- Re-export essential types at crate root
- Use type aliases for complex generic types
- Implement builder patterns for complex constructors
- Provide both sync and async APIs where applicable

## Error Handling Standards

### Error Type Design

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },
    
    #[error("Operation timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },
}
```

### Error Handling Patterns

- Use `thiserror` for library error types
- Use `anyhow` for application error handling
- Provide context with error messages
- Implement `From` traits for error conversion
- Never use `unwrap()` or `expect()` in library code
- Use `Result<T>` return types consistently

## Async Programming Standards

### Runtime Choice

- **Primary**: tokio with full features for applications
- **Library**: Avoid runtime-specific code, use async-trait
- **Testing**: Use tokio-test for async test utilities

### Async Patterns

```rust
use async_trait::async_trait;

#[async_trait]
pub trait AsyncProcessor {
    type Item;
    type Error;
    
    async fn process(&mut self, item: Self::Item) -> Result<(), Self::Error>;
}

// Cancellation-safe async functions
pub async fn process_with_timeout<T>(
    future: impl Future<Output = T>,
    timeout: Duration,
) -> Result<T, TimeoutError> {
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| TimeoutError)
}
```

## Performance Standards

### Memory Management

- Prefer stack allocation over heap allocation
- Use `SmallVec` for collections that are usually small
- Use `Arc` for shared ownership, `Rc` for single-threaded
- Implement `Clone` efficiently with `Arc<T>` for expensive types
- Use `Cow` for conditional cloning scenarios

### Optimization Guidelines

- Profile before optimizing
- Use `#[inline]` judiciously for hot paths
- Prefer iterators over manual loops
- Use `rayon` for data parallelism
- Implement zero-copy APIs where possible

### Benchmarking Requirements

```rust
// Every performance-critical function must have benchmarks
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_critical_function(c: &mut Criterion) {
    c.bench_function("critical_function", |b| {
        b.iter(|| {
            critical_function(black_box(test_data()))
        })
    });
}
```

## Security and Safety Standards

### Memory Safety

- **Forbidden**: `unsafe` code except in designated modules
- Use `Pin` for self-referential types
- Validate all inputs at API boundaries
- Use bounds checking explicitly when needed

### Security Practices

```toml
# deny.toml - Security constraints
[advisories]
vulnerability = "deny"
unmaintained = "warn" 
yanked = "warn"

[bans]
deny = [
    { name = "openssl", version = "<0.10.55" },  # CVE fixes
]
```

## Testing Standards

### Test Organization

```rust
// Unit tests in same file
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = create_test_input();
        
        // Act  
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_output());
    }
}

// Integration tests in tests/ directory
// tests/integration_test.rs
```

### Testing Requirements

- **Coverage Target**: Minimum 90% line coverage
- **Property Testing**: Use `proptest` for complex algorithms
- **Fuzzing**: Use `cargo-fuzz` for input parsing code
- **Doctests**: All public APIs must have working examples

### Test Utilities

```rust
// Test helper functions
#[cfg(test)]
pub fn create_test_audio_frame() -> AudioFrame {
    let samples = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
    AudioFrame::new(44100, 2, samples, Utc::now()).unwrap()
}
```

## Documentation Standards

### API Documentation

```rust
/// Processes audio frames with the specified parameters.
///
/// This function applies signal processing to the input audio frame
/// according to the provided parameters.
///
/// # Arguments
///
/// * `frame` - The input audio frame to process
/// * `params` - Processing parameters including gain and filters
///
/// # Returns
///
/// Returns a processed audio frame or an error if processing fails.
///
/// # Errors
///
/// * `Error::InvalidInput` - If the input frame is malformed
/// * `Error::ProcessingFailed` - If signal processing fails
///
/// # Examples
///
/// ```rust
/// use sdrtrunk_core::{AudioFrame, ProcessingParams};
/// 
/// let frame = AudioFrame::new(44100, 2, samples, timestamp)?;
/// let params = ProcessingParams::default();
/// let processed = process_audio_frame(frame, params)?;
/// ```
///
/// # Panics
///
/// This function does not panic under normal circumstances.
///
/// # Safety
///
/// This function is safe to call from any context.
pub fn process_audio_frame(
    frame: AudioFrame, 
    params: ProcessingParams
) -> Result<AudioFrame> {
    // Implementation
}
```

## Build and CI/CD Standards

### Cargo Configuration

- Use workspace inheritance for common metadata
- Pin exact versions in `Cargo.lock`
- Use feature flags for optional functionality
- Configure release profiles for optimization

### GitHub Actions Pipeline

1. **Format Check**: `cargo fmt --check`
2. **Linting**: `cargo clippy` with strict settings
3. **Compilation**: Multi-platform builds (Linux, macOS, Windows)
4. **Testing**: `cargo nextest run` with full feature matrix
5. **Coverage**: Generate and upload coverage reports
6. **Security**: `cargo audit` and `cargo deny` checks
7. **Documentation**: Build and deploy docs
8. **Benchmarks**: Performance regression detection

### Release Process

1. Update version numbers across workspace
2. Update CHANGELOG.md with notable changes
3. Run full test suite and benchmarks
4. Tag release with semantic versioning
5. Publish crates in dependency order
6. Deploy documentation updates

## Code Quality Enforcement

### Automated Checks

```bash
# Pre-commit hooks
just pre-commit  # Runs formatting, linting, tests

# CI pipeline equivalents  
just ci         # Full CI simulation locally
```

### Metrics Tracking

- **Performance**: Benchmark regression detection
- **Security**: Weekly vulnerability scans
- **Dependencies**: Monthly dependency updates
- **Coverage**: Track coverage trends over time

## Dependency Management

### Dependency Categories

- **Core**: Essential dependencies with strong maintenance
- **Optional**: Feature-gated dependencies
- **Dev-only**: Development and testing tools
- **Build**: Build-time dependencies only

### Version Strategy

- Use caret requirements (`^1.0`) for stable APIs
- Pin exact versions for security-sensitive dependencies
- Regular dependency audits and updates
- Minimize dependency count through careful selection

### Licensing Compliance

- Allow: MIT, Apache-2.0, BSD variants, ISC
- Review: MPL-2.0, CC licenses
- Deny: GPL variants, proprietary licenses
