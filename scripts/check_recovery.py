#!/usr/bin/env python3
"""Exercise the real CLI/store with synthetic edition fixtures."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
CORE = HERE/'../target/debug/reader-core'
FIXTURES = HERE/'fixtures'

def main():
    (HERE/'.local').mkdir(exist_ok=True)
    results = []
    with tempfile.TemporaryDirectory(dir=HERE/'.local') as root:
        root = Path(root)
        epub, store, export = root/'edition.epub', root/'store', root/'book.json'
        def run(*args):
            return subprocess.check_output([str(CORE), *args, '--epub',str(epub),'--store',str(store)],text=True).strip()
        shutil.copyfile(FIXTURES/'edition_original.epub',epub)
        run('export','--output',str(export))
        book = json.loads(export.read_text())
        amber = next(p for p in book['pieces'] if p['text']=='amber')
        run('save','--piece',amber['id'])
        saved = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in store.rglob('*.json')}
        for name, expected in [('original','exact'),('unique','propose_chapter'),('ambiguous','propose_chapter'),('chapter_only','propose_chapter'),('no_match','preserve')]:
            shutil.copyfile(FIXTURES/f'edition_{name}.epub',epub)
            actual = run('restore')
            assert actual.startswith(expected), (name,actual)
            after = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in store.rglob('*.json')}
            assert after == saved, 'Offering recovery mutated saved location'
            results.append({'edition':name,'result':actual,'saved_unchanged':True})
    report = {'results':results,'limitation':'The four-word passage is below the Rust core confidence threshold of eight words/32 scalars, so unique also offers the identified chapter. Longer unique passage recovery is covered by core tests; both native frontends share the core recovery policy.'}
    (HERE/'evidence').mkdir(exist_ok=True)
    (HERE/'evidence/recovery.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))

if __name__ == '__main__':
    main()
