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
notes = json.loads(pathlib.Path('fixtures/demo.json').read_text())
notes['book_id'] = 'qt-notes-parity'
notes['pieces'][3]['kind'] = 'note'
notes['pieces'][3]['note_id'] = 'n'
notes['notes'] = [{'id':'n', 'blocks':[{'id':'bn','kind':'paragraph','runs':[{'text':'Note, done.'}]}],
    'pieces':[{'id':'n1','block_id':'bn','text':'Note,','kind':'word'}, {'id':'n2','block_id':'bn','text':'done.','kind':'word'}]}]
for document in notes['documents']:
    for block in document['blocks']:
        if block.get('resource'):
            block['resource'] = str(pathlib.Path('fixtures', block['resource']).resolve())
(state / 'notes.json').write_text(json.dumps(notes))
(state / 'before.json').write_bytes((state / 'resume.json').read_bytes())
span = {"document":"d", "node":"n", "offset":0, "length":17}
compound = {"schema_version":1,"book_id":"compound-cache","revision":"v1","title":"Compound",
    "documents":[{"id":"d","blocks":[{"id":"b","kind":"paragraph","runs":[{"text":"state-of-the-art.","sources":[span]}]}]}],
    "pieces":[{"id":"p","text":"state-of-the-art.","kind":"word","word_id":"w","block_id":"b","paragraph_end":True,"sources":[span]}],
    "chapters":[],"notes":[]}
(state / 'compound.json').write_text(json.dumps(compound))
PYFIXTURE
READER_SMOKE=recover-pending "$reader_binary" --book-json "$state/changed.json" 2>&1 | tee evidence/runtime/recovery-pending.log
cmp "$state/before.json" "$state/resume.json"
READER_SMOKE=recover-confirm "$reader_binary" --book-json "$state/changed.json" 2>&1 | tee evidence/runtime/recovery-confirm.log
READER_SMOKE=resume-chapter "$reader_binary" 2>&1 | tee evidence/runtime/recovered-resume.log
READER_STATE_DIR="$state/selection" READER_SMOKE=selection READER_SCREENSHOT="$PWD/evidence/selection.png" "$reader_binary" --book-json "$PWD/fixtures/demo.json" 2>&1 | tee evidence/runtime/selection.log
READER_STATE_DIR="$state/compound" READER_SMOKE=compound "$reader_binary" --book-json "$state/compound.json" 2>&1 | tee evidence/runtime/compound.log
if rg -n 'ReferenceError|TypeError|Unable to assign|SMOKE FAIL|QQmlApplicationEngine failed' evidence/runtime/{smoke,resume,recovery-pending,recovery-confirm,recovered-resume,selection,compound}.log; then exit 1; fi

rg -q "SMOKE COMPLETE" evidence/runtime/smoke.log
rg -q "RESUME COMPLETE" evidence/runtime/resume.log
rg -q "RECOVERY_PENDING COMPLETE" evidence/runtime/recovery-pending.log
rg -q "RECOVERY_CONFIRM COMPLETE" evidence/runtime/recovery-confirm.log
rg -q "RESUME COMPLETE" evidence/runtime/recovered-resume.log
rg -q "READER_PLATFORM=$QT_QPA_PLATFORM" evidence/runtime/smoke.log

rg -q "SELECTION COMPLETE" evidence/runtime/selection.log
rg -q "COMPOUND COMPLETE" evidence/runtime/compound.log

READER_STATE_DIR="$state/parity" READER_SMOKE=parity READER_SCREENSHOT="$PWD/evidence/sidebar.png" "$reader_binary" --book-json "$PWD/fixtures/demo.json" 2>&1 | tee evidence/runtime/parity.log
rg -q "PARITY COMPLETE" evidence/runtime/parity.log
if rg -n 'ReferenceError|TypeError|Unable to assign|SMOKE FAIL|QQmlApplicationEngine failed' evidence/runtime/parity.log; then exit 1; fi

READER_STATE_DIR="$state/notes" READER_SMOKE=notes "$reader_binary" --book-json "$state/notes.json" 2>&1 | tee evidence/runtime/notes.log
rg -q "NOTES COMPLETE" evidence/runtime/notes.log
if rg -n 'ReferenceError|TypeError|Unable to assign|SMOKE FAIL|QQmlApplicationEngine failed' evidence/runtime/notes.log; then exit 1; fi
