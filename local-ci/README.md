# Local CI/CD Environment

Run the complete CI/CD pipeline locally using Podman containers before pushing to GitHub.

## Features

- Full PostgreSQL test database (no test skipping!)
- Runs all GitHub Actions CI checks locally
- Caches dependencies for faster subsequent runs
- Provides detailed feedback on failures
- Supports optional advanced checks (coverage, benchmarks, Miri)

## Prerequisites

- Podman installed ([Installation Guide](https://podman.io/getting-started/installation))
- podman-compose (`pip install podman-compose`) or docker-compose

## Quick Start

```bash
# Run standard CI checks (format, clippy, tests, docs)
./run-local-ci.sh

# Run quick checks only (format, clippy, basic tests)
./run-local-ci.sh --quick

# Include code coverage analysis
./run-local-ci.sh --coverage

# Run everything including benchmarks and Miri
./run-local-ci.sh --coverage --benchmarks --miri

# Clean rebuild (removes containers and caches)
./run-local-ci.sh --clean
```

## What Gets Tested

### Standard Checks (Always Run)
1. **Formatting** - `cargo fmt --check`
2. **Compilation** - All targets and features
3. **Clippy Linting** - With warnings as errors
4. **Unit Tests** - Using cargo-nextest
5. **Doc Tests** - All documentation examples
6. **Integration Tests** - Including database tests (no skipping!)
7. **Documentation Build** - Ensures docs compile
8. **Security Audit** - Checks for known vulnerabilities
9. **Dependency Check** - Finds unused dependencies

### Optional Checks
- **Code Coverage** (`--coverage`) - Generates coverage report
- **Benchmarks** (`--benchmarks`) - Runs performance benchmarks
- **Miri** (`--miri`) - Memory safety analysis for unsafe code

## Directory Structure

```
local-ci/
├── containers/           # Container definitions
│   ├── Containerfile.ci      # Rust CI environment
│   └── Containerfile.postgres # Test database
├── config/              # Configuration files
│   └── init-test-db.sql     # Database initialization
├── scripts/             # CI scripts
│   └── run-ci-checks.sh     # Main CI check runner
├── docker-compose.yml   # Service orchestration
└── run-local-ci.sh     # Entry point script
```

## Database Testing

The test database is automatically:
- Created with proper schema and permissions
- Initialized with required extensions (uuid-ossp, pg_trgm)
- Made available at `TEST_DATABASE_URL` for integration tests
- Cleaned up after tests complete

All database integration tests run with a real PostgreSQL instance - no tests are skipped!

## Troubleshooting

### Container Issues
```bash
# Clean up all containers and volumes
podman-compose down -v
podman system prune -f

# Rebuild containers
podman-compose build --no-cache
```

### Permission Issues
If you get permission errors with volumes:
```bash
# Add :z flag to volumes in docker-compose.yml (already included)
# Or run with --privileged flag
podman run --privileged ...
```

### Database Connection Issues
The test database is exposed on port 5433 (to avoid conflicts with local PostgreSQL):
```bash
# Test connection from host
psql -h localhost -p 5433 -U sdrtrunk_test -d sdrtrunk_test
# Password: test_password
```

## CI/CD Workflow

1. Make your code changes
2. Run `./run-local-ci.sh` to test locally
3. Fix any issues found
4. Commit and push to GitHub
5. GitHub Actions will run the same checks

## Tips

- Use `--quick` mode during development for faster feedback
- Run full checks before committing
- The first run will be slower (building containers and downloading dependencies)
- Subsequent runs use cached dependencies for speed
- Check `lcov.info` for detailed coverage reports when using `--coverage`

## Environment Variables

You can customize behavior with these environment variables:
- `RUN_COVERAGE=true` - Enable coverage analysis
- `RUN_BENCHMARKS=true` - Run benchmarks
- `RUN_MIRI=true` - Run Miri checks

## Integration with Development Workflow

```bash
# During development
./run-local-ci.sh --quick

# Before committing
./run-local-ci.sh

# For comprehensive analysis
./run-local-ci.sh --coverage --benchmarks

# After major changes
./run-local-ci.sh --clean --coverage
```