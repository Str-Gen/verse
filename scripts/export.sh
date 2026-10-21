#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cargo +1.99.0 build --manifest-path "$here/../crates/reader-cli/Cargo.toml" --locked -j 2
exec "$here/../target/debug/reader-core" "$@"
