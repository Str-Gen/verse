#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cargo +1.99.0 build --manifest-path "$here/../core/Cargo.toml" --locked -j 2
exec "$here/../core/target/debug/reader-core" "$@"
