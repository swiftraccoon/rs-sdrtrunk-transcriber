#!/bin/bash
# Convenience wrapper for local CI checks

cd "$(dirname "$0")"
exec ./local-ci/run-local-ci.sh "$@"