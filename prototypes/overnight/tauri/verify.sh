#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export CARGO_BUILD_JOBS=2
export TYPESAFE_API_KEY="" VERSE_KEY_FILE=/nonexistent/verse-test-key
cargo +1.99.0 fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo +1.99.0 clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo +1.99.0 test --manifest-path src-tauri/Cargo.toml --workspace --locked
node --test tests/*.test.mjs
./build.sh
mkdir -p .local
export READER_SCRATCH
READER_SCRATCH="$(mktemp -d "$PWD/.local/verification.XXXXXX")"
export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
for attempt in 1 2; do
  ./src-tauri/target/debug/quiet-reader-tauri --self-test
  python - "$READER_SCRATCH/self-test/self-test.json" "$attempt" <<'PY'
import json,sys
result=json.load(open(sys.argv[1]))
assert result['passed'], result['errors']
print(f"Native run {sys.argv[2]}: {len(result['checks'])} checks passed")
PY
done

python tests/theme.py
