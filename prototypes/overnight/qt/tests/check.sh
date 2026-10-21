#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS=2
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings
cargo test --workspace --locked -j 2
cargo test --workspace --doc --locked -j 2
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked -j 2
./tests/theme.sh
./tests/whitespace.sh
./tests/smoke.sh
