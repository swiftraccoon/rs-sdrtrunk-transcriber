# Justfile for Rust project automation
# Install just: cargo install just
# Usage: just <recipe>

# Default recipe - show available commands
default:
    @just --list

# Install required development tools
install-tools:
    cargo install just --locked
    cargo install cargo-audit --locked
    cargo install cargo-deny --locked
    cargo install cargo-machete --locked
    cargo install cargo-nextest --locked
    cargo install cargo-llvm-cov --locked
    cargo install sqlx-cli --no-default-features --features rustls,postgres --locked

# Development setup (run once after cloning)
setup:
    rustup component add rustfmt clippy rust-src rust-analyzer llvm-tools-preview
    just install-tools
    cargo check --workspace

# Format code (use --check to verify without modifying)
fmt *args:
    cargo fmt --all {{args}}

# Run clippy lints (use CLIPPY_FLAGS env var for different strictness levels)
lint flags="":
    #!/usr/bin/env bash
    if [ -n "{{flags}}" ]; then
        CLIPPY_FLAGS="{{flags}}"
    elif [ -z "$CLIPPY_FLAGS" ]; then
        CLIPPY_FLAGS="-D warnings"
    fi
    cargo clippy --workspace --all-targets --all-features -- $CLIPPY_FLAGS

# Check code compilation
check:
    cargo check --workspace --all-targets --all-features

# Run tests (use TEST_ARGS env var or args for filtering)
test args="":
    cargo nextest run --workspace --all-features {{args}}

# Run doc tests
test-doc:
    cargo test --doc --workspace --all-features

# Generate test coverage (use --html for HTML report, --lcov for lcov format)
coverage args="":
    #!/usr/bin/env bash
    if [[ "{{args}}" == *"--html"* ]]; then
        cargo llvm-cov nextest --workspace --all-features --html --fail-under 90
        open target/llvm-cov/html/index.html
    elif [[ "{{args}}" == *"--lcov"* ]]; then
        cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info
    else
        cargo llvm-cov nextest --workspace --all-features --fail-under 90
    fi

# Build in release mode
build:
    cargo build --workspace --release

# Build documentation (use --no-open to skip browser)
docs *args:
    cargo doc --workspace --all-features --no-deps {{args}}

# Clean build artifacts
clean:
    cargo clean

# Security audit
audit:
    cargo audit --deny warnings

# Check dependencies with cargo-deny
deny:
    cargo deny check

# Find unused dependencies
unused-deps:
    cargo machete

# Run CI checks locally (matches GitHub Actions)
ci:
    just fmt --check
    just check
    just lint
    just lint "pedantic"
    just test
    just test-doc
    just docs --no-open
    just audit
    just deny
    just unused-deps
    just coverage

# Run quick development checks
dev:
    just fmt
    just check
    just test

# Run pre-commit checks
pre-commit:
    just fmt
    just lint
    just test

# Database setup for local testing
db-setup:
    docker run -d --name sdrtrunk-test-db \
        -e POSTGRES_USER=sdrtrunk_test \
        -e POSTGRES_PASSWORD=test_password \
        -e POSTGRES_DB=sdrtrunk_test \
        -p 5433:5432 \
        postgres:16-alpine || docker start sdrtrunk-test-db

# Run database migrations
db-migrate:
    cd crates/sdrtrunk-database && \
    sqlx migrate run --database-url "postgresql://sdrtrunk_test:test_password@localhost:5433/sdrtrunk_test"

# Stop test database
db-stop:
    docker stop sdrtrunk-test-db || true

# Clean up test database
db-clean:
    docker stop sdrtrunk-test-db || true
    docker rm sdrtrunk-test-db || true

# Run containerized CI environment
ci-local:
    ./check-ci.sh

# Run containerized CI with coverage
ci-full:
    ./check-ci.sh --coverage --benchmarks

# Update all tools and dependencies
update:
    rustup update
    cargo update
    just install-tools

# Aliases for common commands
alias f := fmt
alias l := lint
alias t := test
alias c := check
alias b := build
alias d := docs

# Environment-specific clippy presets
lint-pedantic:
    @CLIPPY_FLAGS="-D warnings -W clippy::pedantic -A clippy::missing_errors_doc -A clippy::missing_panics_doc" just lint

lint-strict:
    @CLIPPY_FLAGS="-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo -A clippy::multiple_crate_versions" just lint