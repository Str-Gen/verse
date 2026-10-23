#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p evidence
state=$(mktemp -d)
trap 'rm -rf "$state"' EXIT
export TYPESAFE_API_KEY="" VERSE_KEY_FILE=/nonexistent/verse-test-key
export VERSE_DICTIONARY="$state/dictionary.sqlite"
python - "$state" <<'PY'
import json, sys
from pathlib import Path
sys.path.insert(0,str(Path('../../crates/reader-core/vocabulary').resolve()))
from dictionary import build
root=Path(sys.argv[1]); source=root/'dictionary.jsonl'
source.write_text(json.dumps(dict(word='crossed',lang_code='en',pos='verb',senses=[{'glosses':['Went across.']},{'glosses':['Intersected.']}]))+'\n')
build(source,root/'dictionary.sqlite')
PY
reader_binary=${READER_BINARY:-../../target/debug/verse-qt}
if [[ -z "${READER_BINARY:-}" ]]; then cargo build --locked -j 2; fi
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software READER_STATE_DIR="$state" READER_SMOKE=experience READER_SCREENSHOT="$PWD/evidence/vocabulary.png" "$reader_binary" --book-json "$PWD/fixtures/demo.json" 2>&1 | tee "$state/result.log"
! rg 'SMOKE FAIL|ReferenceError|TypeError|Unable to assign|QQmlApplicationEngine failed' "$state/result.log"
rg 'EXPERIENCE COMPLETE' "$state/result.log"
