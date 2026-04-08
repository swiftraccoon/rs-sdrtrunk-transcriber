#!/usr/bin/env bash
# lint.sh - Strict quality enforcement
set -euo pipefail

cd "$(dirname "$0")"

failed=0

run() {
    echo "── $* ──"
    if ! "$@"; then
        failed=1
    fi
}

# Unsafe code audit — flag any allow(unsafe_code) that isn't whitelisted
echo "── unsafe code audit ──"
unsafe_allows=$(grep -rn 'allow(unsafe_code)' crates/ --include="*.rs" || true)
if [ -n "$unsafe_allows" ]; then
    echo "ERROR: Found allow(unsafe_code) directives:"
    echo "$unsafe_allows"
    failed=1
fi

# Clippy with fail-on-warnings
run cargo clippy --workspace --all-targets -- -D warnings

# Doc generation with fail-on-warnings
RUSTDOCFLAGS="-D warnings" run cargo doc --workspace --no-deps

# All tests must pass
run cargo test --workspace

# Format check (require manual fmt, don't auto-format)
run cargo fmt --all -- --check

# Security audit (optional - skip if not installed)
if command -v cargo-audit &>/dev/null; then
    run cargo audit --ignore RUSTSEC-2025-0111
else
    echo "── cargo-audit not installed, skipping (cargo install cargo-audit) ──"
fi

# Dependency/license validation (optional)
if command -v cargo-deny &>/dev/null; then
    run cargo deny check
else
    echo "── cargo-deny not installed, skipping (cargo install cargo-deny) ──"
fi

# Unused dependency detection (optional)
if command -v cargo-machete &>/dev/null; then
    run cargo machete
else
    echo "── cargo-machete not installed, skipping (cargo install cargo-machete) ──"
fi

if [ "$failed" -ne 0 ]; then
    echo "FAILED"
    exit 1
fi
echo "OK"
