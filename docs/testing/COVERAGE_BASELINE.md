# Test Coverage Baseline - Phase 1

**Date**: 2025-10-07
**Branch**: `chore/test-infrastructure-refactor-phase1`
**Measurement Tool**: `cargo llvm-cov nextest --workspace --all-features`

## Overall Coverage

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| **Line Coverage** | **74.48%** | ≥90% | ❌ Below target (-15.52%) |
| **Function Coverage** | 75.26% | N/A | ℹ️ Reference |
| **Region Coverage** | 74.48% | N/A | ℹ️ Reference |
| **Total Lines** | 16,303 | N/A | - |
| **Missed Lines** | 4,082 | N/A | - |

## Per-Crate Coverage Analysis

### ✅ High Coverage (≥90%)

These crates meet or exceed the 90% coverage target:

| Crate | Line Coverage | Status |
|-------|---------------|--------|
| `sdrtrunk-core/src/lib.rs` | 100.00% | ✅ Excellent |
| `sdrtrunk-core/src/types.rs` | 98.85% | ✅ Excellent |
| `sdrtrunk-core/src/config.rs` | 98.68% | ✅ Excellent |
| `sdrtrunk-core/src/utils.rs` | 98.71% | ✅ Excellent |
| `sdrtrunk-core/src/error.rs` | 98.33% | ✅ Excellent |
| `sdrtrunk-api/src/state.rs` | 98.08% | ✅ Excellent |
| `sdrtrunk-api/src/routes.rs` | 94.27% | ✅ Good |
| `sdrtrunk-monitor/src/config.rs` | 98.11% | ✅ Excellent |
| `sdrtrunk-monitor/src/error.rs` | 99.64% | ✅ Excellent |
| `sdrtrunk-monitor/src/lib.rs` | 96.03% | ✅ Excellent |
| `sdrtrunk-monitor/src/queue.rs` | 95.91% | ✅ Excellent |
| `sdrtrunk-transcriber/src/service.rs` | 100.00% | ✅ Excellent |
| `sdrtrunk-transcriber/src/mock.rs` | 93.48% | ✅ Good |
| `sdrtrunk-database/src/lib.rs` | 93.62% | ✅ Good |
| `sdrtrunk-core/src/lazy.rs` | 92.11% | ✅ Good |

### ⚠️ Medium Coverage (70-89%)

These crates are below target and need improvement:

| Crate | Line Coverage | Gap | Priority |
|-------|---------------|-----|----------|
| `sdrtrunk-transcriber/src/types.rs` | 88.33% | -1.67% | Low |
| `sdrtrunk-api/src/handlers/stats.rs` | 83.40% | -6.60% | Medium |
| `sdrtrunk-api/src/handlers/audio_utils.rs` | 75.66% | -14.34% | High |
| `sdrtrunk-api/src/lib.rs` | 75.48% | -14.52% | High |
| `sdrtrunk-monitor/src/monitor.rs` | 75.72% | -14.28% | High |
| `sdrtrunk-api/src/handlers/calls.rs` | 69.89% | -20.11% | High |
| `sdrtrunk-api/src/handlers/health.rs` | 72.84% | -17.16% | High |
| `sdrtrunk-api/src/handlers/upload.rs` | 69.62% | -20.38% | Critical |
| `sdrtrunk-monitor/src/main.rs` | 79.82% | -10.18% | Medium |
| `sdrtrunk-monitor/src/processor.rs` | 78.65% | -11.35% | High |
| `sdrtrunk-monitor/src/service.rs` | 68.29% | -21.71% | Critical |

### ❌ Low Coverage (<70%)

These crates have critical coverage gaps:

| Crate | Line Coverage | Gap | Priority |
|-------|---------------|-----|----------|
| `sdrtrunk-database/src/queries.rs` | **65.53%** | -24.47% | **CRITICAL** |
| `sdrtrunk-transcriber/src/error.rs` | 64.60% | -25.40% | Critical |
| `sdrtrunk-api/src/main.rs` | 53.57% | -36.43% | Critical |

### 🚫 Zero Coverage (0%)

These files have no test coverage (not integrated or untested):

| Crate | Lines | Reason |
|-------|-------|--------|
| `sdrtrunk-api/src/handlers/transcription.rs` | 67 | Handler not fully implemented |
| `sdrtrunk-transcriber/src/whisperx.rs` | 149 | WhisperX integration code |
| `sdrtrunk-transcriber/src/whisperx_impl.rs` | 27 | WhisperX implementation details |
| `sdrtrunk-transcriber/src/worker.rs` | 198 | Worker pool (async runtime) |
| `sdrtrunk-web/src/api_client.rs` | 124 | Web UI API client (frontend) |
| `sdrtrunk-web/src/handlers/api.rs` | 132 | Web UI handlers (frontend) |
| `sdrtrunk-web/src/handlers/pages.rs` | 16 | Web UI page handlers |
| `sdrtrunk-web/src/main.rs` | 19 | Web UI main entry point |
| `sdrtrunk-web/src/routes.rs` | 12 | Web UI routes |
| `sdrtrunk-web/src/server.rs` | 4 | Web UI server setup |
| `sdrtrunk-web/src/state.rs` | 5 | Web UI state management |
| `sdrtrunk-web/src/websocket.rs` | 35 | WebSocket handler |

## Coverage Gaps Analysis

### Critical Priorities (Phases 2-4)

1. **Database Queries** (`sdrtrunk-database/src/queries.rs`): 65.53%
   - 1,221 lines uncovered
   - Critical for data integrity
   - **Action**: Add comprehensive query tests (error paths, edge cases)

2. **API Upload Handler** (`sdrtrunk-api/src/handlers/upload.rs`): 69.62%
   - 528 lines uncovered
   - Security-critical input validation
   - **Action**: Add negative tests, property tests for parsing

3. **Monitor Service** (`sdrtrunk-monitor/src/service.rs`): 68.29%
   - 261 lines uncovered
   - File monitoring logic
   - **Action**: Add error path tests, concurrent access tests

4. **Transcription Handler** (`sdrtrunk-api/src/handlers/transcription.rs`): 0%
   - Not yet integrated
   - **Action**: Complete implementation, add tests

### Medium Priorities

5. **API Main Entry** (`sdrtrunk-api/src/main.rs`): 53.57%
   - Server initialization code
   - **Action**: Add startup/shutdown tests

6. **Monitor Processor** (`sdrtrunk-monitor/src/processor.rs`): 78.65%
   - Audio file processing
   - **Action**: Add edge case tests for file formats

7. **API Stats Handler** (`sdrtrunk-api/src/handlers/stats.rs`): 83.40%
   - Close to target
   - **Action**: Add a few edge case tests

### Deferred (Out of Scope)

8. **Web UI Crates** (`sdrtrunk-web/*`): 0%
   - Frontend code requires browser testing
   - **Action**: Future work with Playwright/Selenium

9. **WhisperX Integration** (`sdrtrunk-transcriber/src/whisperx*.rs`): 0%
   - External Python service integration
   - **Action**: Integration tests in Phase 2

## Test Quality Improvements (Phase 1 Complete)

### ✅ Completed in Phase 1

- **Fixed weak assertions** in `tests/integration_api.rs`:
  - Line 106: Replaced vague status check with specific `StatusCode::OK`
  - Line 191: Replaced weak client error check with specific `StatusCode::NOT_FOUND`

- **Removed manual table creation** in `tests/integration_database.rs`:
  - Lines 88-125: Deleted 30+ lines of SQL table creation
  - Now relies on migrations from `crates/sdrtrunk-database/migrations/`
  - Added verification that migration creates correct schema

- **Baseline coverage documented**:
  - Overall: 74.48% (15.52% below target)
  - Identified 3 critical gaps (database, upload, monitor service)
  - Identified 12 zero-coverage files (mostly web UI and WhisperX)

## Next Steps (Phases 2-4)

### Phase 2 (Weeks 2-3): Property & Fuzz Tests

**Goals**:
- Add 10+ property tests for parsers and validators
- Initialize cargo-fuzz with 5 targets
- Add comprehensive negative test suite

**Expected Coverage Impact**: +5-10% (from property tests finding edge cases)

### Phase 3 (Weeks 4-5): Reach 90% Coverage

**Goals**:
- Add tests for uncovered lines in critical crates
- Focus on error paths and edge cases
- Add performance assertions

**Expected Coverage Impact**: +10-15% (systematic gap filling)

### Phase 4 (Ongoing): Documentation & Maintenance

**Goals**:
- Add doc tests to all public APIs
- Set up mutation testing (85% target)
- Continuous monitoring and improvement

**Expected Coverage Impact**: +5% (doc tests for public APIs)

## Validation Commands

```bash
# Measure coverage
cargo llvm-cov nextest --workspace --all-features

# Generate HTML report
cargo llvm-cov nextest --workspace --all-features --html
open target/llvm-cov/html/index.html

# Generate LCOV for CI
cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info

# Check specific crate
cargo llvm-cov nextest -p sdrtrunk-database --html
```

## Comparison to Target

| Metric | Current | Target | Gap | Status |
|--------|---------|--------|-----|--------|
| Overall Line Coverage | 74.48% | 90% | -15.52% | ❌ Below |
| Crates ≥90% | 15/33 | 33/33 | 18 crates | ⚠️ Partial |
| Crates <70% | 5/33 | 0/33 | 5 crates | ❌ Critical |
| Zero Coverage Files | 12 | 0 | 12 files | ⚠️ Expected |

## Notes

**Zero Coverage Files**: Most zero-coverage files are expected:
- **Web UI crates** (`sdrtrunk-web/*`): Frontend code requiring browser testing
- **WhisperX integration**: External Python service requiring integration tests
- **Worker pool**: Async runtime code requiring complex integration tests

**Realistic Target**: Excluding web UI and external integrations, achievable target is **85-90%** for core library and API crates.

**Phase 1 Impact**: Test quality improvements (weak assertions fixed, manual table creation removed) lay foundation for future coverage improvements without directly increasing coverage percentage.

---

**Created**: 2025-10-07
**Author**: Test Infrastructure Refactor - Phase 1
**Next Review**: After Phase 2 completion (Weeks 2-3)
