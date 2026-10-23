#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
reader_binary=${READER_BINARY:-../../target/debug/verse-qt}
if [[ -z "${READER_BINARY:-}" ]]; then cargo build --locked -p verse-qt -j 2; fi
state=$(mktemp -d)
trap 'rm -rf "$state"' EXIT
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=${READER_TEST_PLATFORM:-offscreen} QT_QUICK_BACKEND=software QT_SCALE_FACTOR=1 \
  READER_THEME_FILE="$PWD/fixtures/theme-light.toml" READER_STATE_DIR="$state" \
  READER_SMOKE=loading "$reader_binary" --book-json "$PWD/fixtures/demo.json"
python3 - "$state" <<'PY'
import json, sys
from pathlib import Path
state = Path(sys.argv[1])
bad = state / 'broken.epub'
bad.write_bytes(b'not an EPUB')
(state / 'last-book.json').write_text(json.dumps(str(bad)))
PY
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=${READER_TEST_PLATFORM:-offscreen} QT_QUICK_BACKEND=software QT_SCALE_FACTOR=1 \
  READER_THEME_FILE="$PWD/fixtures/theme-light.toml" READER_STATE_DIR="$state" \
  READER_SMOKE=loading-fallback "$reader_binary"
