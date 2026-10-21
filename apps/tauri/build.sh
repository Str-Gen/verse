#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/src-tauri"
export CARGO_BUILD_JOBS=2
cargo build --locked "$@"
