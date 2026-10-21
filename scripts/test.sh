#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export TYPESAFE_API_KEY="" VERSE_KEY_FILE=/nonexistent/verse-test-key

case "${1:-unit}" in
  setup)
    rustup component add llvm-tools-preview
    cargo install --locked cargo-llvm-cov --version 0.9.1
    cargo install --locked cargo-mutants --version 27.1.0
    ;;
  unit)
    cargo test --workspace --locked
    node --test apps/tauri/tests/*.test.mjs
    python -m unittest discover -s scripts/tests
    python -m unittest discover -s crates/reader-core/vocabulary -p 'test_*.py'
    ;;
  coverage)
    cargo llvm-cov --workspace --locked --html
    cargo llvm-cov report --json --output-path target/llvm-cov/coverage.json
    cargo llvm-cov report --show-missing-lines
    node --experimental-test-coverage --test apps/tauri/tests/*.test.mjs
    ;;
  mutants)
    shift
    cargo mutants --workspace --timeout 120 --build-timeout 600 "$@" -- --locked
    ;;
  native)
    ./apps/qt/tests/check.sh
    ./scripts/check_native.sh
    ;;
  *)
    echo "Usage: $0 [setup|unit|coverage|mutants|native]" >&2
    exit 2
    ;;
esac
