#!/bin/bash
# Local CI/CD test runner script
# Runs all the same checks as GitHub Actions CI

set -e

# Set optimized compilation flags for 14 CPUs and 32GB RAM
export CARGO_BUILD_JOBS=12  # Use 12 of 14 CPUs for building
export CARGO_INCREMENTAL=0  # Clean builds for CI
export RUSTFLAGS="-C codegen-units=4"  # Balance between speed and memory

echo "==========================================
Local CI/CD Test Runner
==========================================
"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track failures and create log directory
FAILED_CHECKS=""
LOG_DIR="/tmp/ci-logs"
mkdir -p "$LOG_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="$LOG_DIR/ci_run_${TIMESTAMP}.log"
SUMMARY_FILE="$LOG_DIR/ci_summary_${TIMESTAMP}.txt"

# Redirect all output to log file as well
exec > >(tee -a "$LOG_FILE")
exec 2>&1

# Function to run a check
run_check() {
    local name="$1"
    local cmd="$2"
    
    echo -e "${YELLOW}Running: $name${NC}"
    echo "========================================" >> "$SUMMARY_FILE"
    echo "$name" >> "$SUMMARY_FILE"
    echo "========================================" >> "$SUMMARY_FILE"
    
    # Create a temporary file for this check's output
    local tmp_output="/tmp/${name// /_}_output.txt"
    
    if eval "$cmd" > "$tmp_output" 2>&1; then
        echo -e "${GREEN}✓ $name passed${NC}\n"
        echo "✓ PASSED" >> "$SUMMARY_FILE"
        # Show only first few lines of successful output
        head -5 "$tmp_output"
        if [ $(wc -l < "$tmp_output") -gt 5 ]; then
            echo "... (output truncated, see log for full details)"
        fi
    else
        local exit_code=$?
        echo -e "${RED}✗ $name failed (exit code: $exit_code)${NC}\n"
        echo "✗ FAILED (exit code: $exit_code)" >> "$SUMMARY_FILE"
        FAILED_CHECKS="$FAILED_CHECKS\n  - $name"
        
        # Show the actual error output
        echo "Error output:"
        cat "$tmp_output"
        # Also save errors to summary
        echo "Key errors:" >> "$SUMMARY_FILE"
        grep -E "error\[|error:|ERROR:|failed|FAILED" "$tmp_output" | head -20 >> "$SUMMARY_FILE" || true
    fi
    
    echo "" >> "$SUMMARY_FILE"
    rm -f "$tmp_output"
}

# Wait for database to be ready
echo "Waiting for database to be ready..."
for i in {1..30}; do
    if pg_isready -h postgres -U sdrtrunk_test -d sdrtrunk_test 2>/dev/null; then
        echo -e "${GREEN}Database is ready!${NC}\n"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}Database failed to start${NC}"
        exit 1
    fi
    sleep 1
done

# Export test database URL for integration tests
export TEST_DATABASE_URL="postgresql://sdrtrunk_test:test_password@postgres:5432/sdrtrunk_test"
export DATABASE_URL="$TEST_DATABASE_URL"

# Run database migrations BEFORE any tests
echo -e "${YELLOW}Setting up database...${NC}"
cd /workspace/crates/sdrtrunk-database
if sqlx migrate run --database-url "$TEST_DATABASE_URL"; then
    echo -e "${GREEN}Database migrations applied successfully${NC}\n"
else
    echo -e "${RED}Migration failed${NC}"
    exit 1
fi
cd /workspace

# 1. Formatting check
run_check "Formatting" "cargo fmt --all -- --check"

# 2. Compilation check  
run_check "Compilation" "cargo check --workspace --all-targets --all-features -j 2"

# 3. Clippy linting (Standard level)
# Allow missing docs for test functions
run_check "Clippy (standard)" "cargo clippy --workspace --all-targets --all-features -j 2 -- -D warnings -A clippy::missing_panics_doc -A clippy::missing_errors_doc"

# 4. Clippy linting (Pedantic level)
# Allow some pedantic lints that are too strict for tests
run_check "Clippy (pedantic)" "cargo clippy --workspace --all-targets --all-features -j 2 -- -D warnings -W clippy::pedantic -A clippy::missing_errors_doc -A clippy::missing_panics_doc -A clippy::too_many_lines -A clippy::cast_possible_wrap -A clippy::cast_possible_truncation -A clippy::module_name_repetitions"

# 5. Run tests with nextest
run_check "Tests (nextest)" "cargo nextest run --workspace --all-features -j 8"

# 6. Run doc tests
run_check "Doc tests" "cargo test --doc --workspace --all-features -j 8"

# 7. Database integration tests (migrations already run at start)
run_check "Database integration tests" "cargo test --package sdrtrunk-database --all-features -j 8"

# 8. Documentation build
run_check "Documentation" "cargo doc --workspace --all-features --no-deps -j 2"

# 9. Security audit
# Copy audit config if it exists
if [ -f "/workspace/.cargo/audit.toml" ]; then
    mkdir -p ~/.cargo
    cp /workspace/.cargo/audit.toml ~/.cargo/audit.toml
fi
run_check "Security audit" "cargo audit --deny warnings"

# 10. Dependency check
run_check "Dependency check" "cargo machete"

# 11. Code coverage (optional, takes longer)
if [ "${RUN_COVERAGE:-false}" = "true" ]; then
    echo -e "${YELLOW}Running: Code Coverage${NC}"
    # Use single job to avoid OOM with coverage instrumentation
    cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info -j 8 --no-fail-fast
    echo -e "${GREEN}Coverage report generated at lcov.info${NC}\n"
fi

# 12. Benchmarks (optional)
if [ "${RUN_BENCHMARKS:-false}" = "true" ]; then
    # Use reduced parallelism for benchmarks too
    run_check "Benchmarks" "CARGO_BUILD_JOBS=2 cargo bench --workspace --all-features --no-run -j 2"
fi

# 13. Miri (optional, for unsafe code checking)
if [ "${RUN_MIRI:-false}" = "true" ]; then
    run_check "Miri (memory safety)" "MIRIFLAGS='-Zmiri-disable-isolation' cargo +nightly miri test --workspace"
fi

# Summary
echo "=========================================="
echo "CI RUN COMPLETE"
echo "=========================================="
echo ""
echo "📄 Full log: $LOG_FILE"
echo "📊 Summary: $SUMMARY_FILE"
echo ""

# Show summary
echo "Check Results:"
echo "--------------"
cat "$SUMMARY_FILE" | grep -E "^✓|^✗|^⚠" | sort | uniq -c

if [ -z "$FAILED_CHECKS" ]; then
    echo ""
    echo -e "${GREEN}All CI checks passed!${NC}"
    echo "Your code is ready to commit."
    exit 0
else
    echo ""
    echo -e "${RED}Some checks failed:${NC}$FAILED_CHECKS"
    echo ""
    echo "To see details of failures, check:"
    echo "  - Summary: local-ci/logs/ci_summary_${TIMESTAMP}.txt"
    echo "  - Full log: local-ci/logs/ci_run_${TIMESTAMP}.log"
    echo ""
    echo "Common issues to check:"
    echo "  - Formatting: cargo fmt --all"
    echo "  - Clippy: cargo clippy --fix"
    echo "  - Tests: cargo test --workspace"
    exit 1
fi