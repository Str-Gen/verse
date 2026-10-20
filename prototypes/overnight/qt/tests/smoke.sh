#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS=2
reader_binary=${READER_BINARY:-target/debug/overnight-qt-reader}
if [[ -z "${READER_BINARY:-}" ]]; then cargo build --locked -j 2; fi
[[ -x "$reader_binary" ]]
mkdir -p .local evidence/runtime
state=$(mktemp -d "$PWD/.local/smoke.XXXXXX")
trap 'rm -rf "$state"' EXIT
export QT_QPA_PLATFORM=${READER_TEST_PLATFORM:-offscreen}
export QT_FORCE_STDERR_LOGGING=1
export QT_SCALE_FACTOR=1
export QSG_RHI_BACKEND=software
export QT_QUICK_BACKEND=software
export READER_THEME_FILE="$PWD/fixtures/theme-light.toml"
export READER_STATE_DIR="$state"
READER_SMOKE=exercise READER_SCREENSHOT="$PWD/evidence/reader.png" "$reader_binary" --book-json "$PWD/fixtures/demo.json" 2>&1 | tee evidence/runtime/smoke.log
READER_SMOKE=resume "$reader_binary" 2>&1 | tee evidence/runtime/resume.log
python - "$state" <<'PYFIXTURE'
import json, pathlib, sys
state = pathlib.Path(sys.argv[1])
book = json.loads(pathlib.Path('fixtures/demo.json').read_text())
book['revision'] = 'synthetic-edited-v2'
for document in book['documents']:
    for block in document['blocks']:
        if block.get('resource'):
            block['resource'] = str(pathlib.Path('fixtures', block['resource']).resolve())
        for run in block.get('runs', []):
            for source in run.get('sources', []):
                source['node'] = 'edited/' + source['node']
for piece in book['pieces']:
    for source in piece.get('sources', []):
        source['node'] = 'edited/' + source['node']
(state / 'changed.json').write_text(json.dumps(book))
(state / 'before.json').write_bytes((state / 'resume.json').read_bytes())
PYFIXTURE
READER_SMOKE=recover-pending "$reader_binary" --book-json "$state/changed.json" 2>&1 | tee evidence/runtime/recovery-pending.log
cmp "$state/before.json" "$state/resume.json"
READER_SMOKE=recover-confirm "$reader_binary" --book-json "$state/changed.json" 2>&1 | tee evidence/runtime/recovery-confirm.log
READER_SMOKE=resume "$reader_binary" 2>&1 | tee evidence/runtime/recovered-resume.log
if rg -n 'ReferenceError|TypeError|Unable to assign|SMOKE FAIL|QQmlApplicationEngine failed' evidence/runtime/{smoke,resume,recovery-pending,recovery-confirm,recovered-resume}.log; then exit 1; fi

rg -q "SMOKE COMPLETE" evidence/runtime/smoke.log
rg -q "RESUME COMPLETE" evidence/runtime/resume.log
rg -q "RECOVERY_PENDING COMPLETE" evidence/runtime/recovery-pending.log
rg -q "RECOVERY_CONFIRM COMPLETE" evidence/runtime/recovery-confirm.log
rg -q "RESUME COMPLETE" evidence/runtime/recovered-resume.log
rg -q "READER_PLATFORM=$QT_QPA_PLATFORM" evidence/runtime/smoke.log
