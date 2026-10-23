#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS=2
cargo fmt --all -- --check
cargo clippy -p verse-qt --all-targets --locked -j 2 -- -D warnings
cargo test -p verse-qt --locked -j 2
cargo test -p verse-qt --doc --locked -j 2
RUSTDOCFLAGS='-D warnings' cargo doc -p verse-qt --no-deps --locked -j 2
./tests/theme.sh
./tests/whitespace.sh
./tests/smoke.sh
./tests/loading.sh

./tests/experience.sh
