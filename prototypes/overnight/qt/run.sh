#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [[ "${1:-}" == "--demo" ]]; then
    shift
    set -- --book-json "$PWD/fixtures/demo.json" "$@"
fi
export CARGO_BUILD_JOBS=2
cargo build --locked -j 2
exec target/debug/overnight-qt-reader "$@"
