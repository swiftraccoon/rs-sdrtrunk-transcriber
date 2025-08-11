# Justfile for Rust project automation
# Install just: cargo install just
# Usage: just <recipe>

# Default recipe
default:
    @just --list

# Install required development tools
install-tools:
    cargo install cargo-audit --locked
    cargo install cargo-deny --locked  
    cargo install cargo-machete --locked
    cargo install cargo-nextest --locked
    cargo install cargo-llvm-cov --locked
    cargo install cargo-expand --locked
    cargo install cargo-udeps --locked
    cargo install cargo-bloat --locked
    cargo install criterion-cli --locked
    cargo install flamegraph --locked

# Format all Rust code
fmt:
    cargo fmt --all

# Check formatting without making changes
fmt-check:
    cargo fmt --all -- --check

# Run clippy with strict settings
lint:
    cargo clippy --workspace --all-targets --all-features -- \
        -D warnings \
        -D clippy::all \
        -D clippy::pedantic \
        -D clippy::nursery \
        -D clippy::cargo \
        -A clippy::multiple_crate_versions

# Run clippy with maximum strictness (including restriction lints)
lint-strict:
    cargo clippy --workspace --all-targets --all-features -- \
        -D warnings \
        -D clippy::all \
        -D clippy::pedantic \
        -D clippy::nursery \
        -D clippy::cargo \
        -D clippy::restriction \
        -A clippy::blanket_clippy_restriction_lints \
        -A clippy::implicit_return \
        -A clippy::missing_docs_in_private_items \
        -A clippy::question_mark_used \
        -A clippy::single_call_fn \
        -A clippy::std_instead_of_alloc \
        -A clippy::std_instead_of_core \
        -A clippy::shadow_reuse \
        -A clippy::shadow_same \
        -A clippy::shadow_unrelated \
        -A clippy::separated_literal_suffix \
        -A clippy::mod_module_files

# Check code compilation
check:
    cargo check --workspace --all-targets --all-features

# Run all tests with nextest
test:
    cargo nextest run --workspace --all-features

# Run tests with standard test runner
test-std:
    cargo test --workspace --all-features

# Run doctests
doctest:
    cargo test --doc --workspace --all-features

# Run tests with Miri for undefined behavior detection
miri:
    cargo +nightly miri test --workspace

# Generate and open test coverage report
coverage:
    cargo llvm-cov nextest --workspace --all-features --html
    open target/llvm-cov/html/index.html

# Generate coverage in lcov format
coverage-lcov:
    cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info

# Security audit
audit:
    cargo audit --deny warnings

# Check dependencies with cargo-deny
deny:
    cargo deny check

# Find unused dependencies
machete:
    cargo machete

# Find unused dependencies (requires nightly)
udeps:
    cargo +nightly udeps --workspace --all-targets

# Build in release mode
build-release:
    cargo build --workspace --release

# Build documentation
docs:
    cargo doc --workspace --all-features --no-deps --open

# Build documentation without opening
docs-build:
    cargo doc --workspace --all-features --no-deps

# Clean build artifacts
clean:
    cargo clean

# Run benchmarks
bench:
    cargo bench --workspace --all-features

# Run specific benchmark
bench-name name:
    cargo bench --workspace --all-features {{name}}

# Profile binary size
bloat:
    cargo bloat --release --crates

# Profile compilation time
timings:
    cargo build --workspace --all-features --timings

# Generate flamegraph for performance profiling (Linux only)
flamegraph:
    cargo flamegraph --bench main_bench

# Expand macros for debugging
expand crate="" file="":
    #!/usr/bin/env bash
    if [ -n "{{crate}}" ]; then
        if [ -n "{{file}}" ]; then
            cargo expand --package {{crate}} --bin {{file}}
        else
            cargo expand --package {{crate}}
        fi
    else
        cargo expand
    fi

# Full quality check (formatting, linting, tests, docs, security)
check-all:
    @echo "🔧 Formatting code..."
    just fmt-check
    @echo "📋 Checking compilation..."
    just check  
    @echo "📎 Running clippy..."
    just lint
    @echo "🧪 Running tests..."
    just test
    @echo "📚 Building docs..."
    just docs-build
    @echo "🔒 Security audit..."
    just audit
    @echo "🚫 Checking dependencies..."
    just deny
    @echo "✅ All checks passed!"

# Pre-commit hook (run before committing)
pre-commit:
    just fmt
    just check-all

# Release preparation
pre-release:
    @echo "🚀 Preparing for release..."
    just check-all
    just lint-strict
    just coverage-lcov
    just machete
    @echo "📦 Building release..."
    just build-release
    @echo "🎯 Ready for release!"

# Development setup (run once after cloning)
setup:
    @echo "🔧 Installing development tools..."
    just install-tools
    @echo "🦀 Setting up Rust..."
    rustup component add rustfmt clippy rust-src rust-analyzer miri llvm-tools-preview
    @echo "📦 Building project..."
    just check
    @echo "✅ Development setup complete!"

# Update all tools and dependencies
update:
    @echo "🔄 Updating Rust toolchain..."
    rustup update
    @echo "🔄 Updating cargo tools..."
    just install-tools
    @echo "🔄 Updating dependencies..."
    cargo update
    @echo "✅ Update complete!"

# Create a new crate in the workspace
new-crate name:
    cargo new --lib crates/{{name}}
    echo '[package]\nname = "{{name}}"\nversion.workspace = true\nedition.workspace = true\nauthors.workspace = true\nlicense.workspace = true\nrepository.workspace = true\nhomepage.workspace = true\ndocumentation.workspace = true\nreadme = "README.md"\ndescription = "{{name}} crate for rs-sdrtrunk-transcriber"\n\n[dependencies]' > crates/{{name}}/Cargo.toml

# Run continuous integration checks locally
ci:
    @echo "🔄 Running CI checks locally..."
    just fmt-check
    just lint-strict  
    just test
    just doctest
    just docs-build
    just audit
    just deny
    just build-release
    @echo "✅ CI checks complete!"

# Development workflow helpers
dev:
    @echo "🔄 Running development checks..."
    just fmt
    just check
    just test
    @echo "✅ Development checks complete!"

# Watch for changes and run tests
watch:
    cargo watch -x "nextest run --workspace --all-features"

# Watch for changes and run checks
watch-check:
    cargo watch -x "check --workspace --all-targets --all-features"