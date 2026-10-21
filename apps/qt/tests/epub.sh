#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
cargo build -p verse-qt --locked -j 2
state=$(mktemp -d)
trap 'rm -rf "$state"' EXIT
export QT_FORCE_STDERR_LOGGING=1
export QT_QPA_PLATFORM=offscreen
export QSG_RHI_BACKEND=software
export QT_QUICK_BACKEND=software
export READER_STATE_DIR="$state"
export READER_SMOKE=epub
./apps/qt/run.sh --epub crates/reader-core/fixtures/complex.epub > "$state/open.log" 2>&1 || { cat "$state/open.log"; exit 1; }
rg 'EPUB COMPLETE' "$state/open.log"
python - "$state/last-book.json" <<'PY'
import json, pathlib, sys
assert pathlib.Path(json.loads(pathlib.Path(sys.argv[1]).read_text())) == pathlib.Path('crates/reader-core/fixtures/complex.epub').resolve()
PY
./target/debug/verse-qt > "$state/reopen.log" 2>&1 || { cat "$state/reopen.log"; exit 1; }
rg 'EPUB COMPLETE' "$state/reopen.log"
if rg 'SMOKE FAIL|ReferenceError|TypeError|QQmlApplicationEngine failed' "$state/open.log" "$state/reopen.log"; then exit 1; fi
