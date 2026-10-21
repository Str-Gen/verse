#!/usr/bin/env python3
"""Exercise real native instance ownership and startup with malformed state on Linux."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qt', type=Path, required=True)
    parser.add_argument('--tauri', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    checks, processes = [], []
    with tempfile.TemporaryDirectory(prefix='reader-instances-') as temporary:
        root = Path(temporary)
        original = json.loads(args.fixture.read_text())
        books = []
        for i in range(3):
            book = {**original, 'book_id': f'instance-test-{uuid.uuid4()}'}
            path = root / f'book-{i}.json'
            path.write_text(json.dumps(book))
            books.append((book['book_id'], path))
        qt_state, tauri_state = root/'qt', root/'tauri'
        qt_state.mkdir(); tauri_state.mkdir()
        (qt_state/'resume.json').write_text('{')
        (tauri_state/'state.json').write_text('{')
        environment = {**os.environ, 'QT_QPA_PLATFORM':'offscreen', 'QT_QUICK_BACKEND':'software',
                       'QT_FORCE_STDERR_LOGGING':'1', 'WEBKIT_DISABLE_DMABUF_RENDERER':'1',
                       'TYPESAFE_API_KEY':'', 'VERSE_KEY_FILE':str(root/'absent')}
        def start(kind, index):
            state = qt_state if kind == 'qt' else tauri_state
            env = {**environment, 'READER_STATE_DIR':str(state), 'READER_SCRATCH':str(state)}
            log = (root/f'process-{len(processes)}.log').open('w+')
            process = subprocess.Popen([str(getattr(args,kind).resolve()), '--book-json',str(books[index][1])],
                                       env=env, stdout=log, stderr=log, start_new_session=True)
            processes.append((process,log))
            return process,log
        def ready(process, log, kind, index):
            state = qt_state if kind == 'qt' else tauri_state
            group = 'resume.books' if kind == 'qt' else 'state.books'
            key = hashlib.sha256(books[index][0].encode()).hexdigest()
            record = state/group/(key+'.json')
            lease = Path.home()/'.local/state/overnight-reader/locks'/(key+'.lock')
            deadline = time.monotonic()+25
            while time.monotonic()<deadline and process.poll() is None:
                if kind == 'qt' and lease.exists():
                    with lease.open('rb') as handle:
                        try: fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        except BlockingIOError: return
                        else: fcntl.flock(handle, fcntl.LOCK_UN)
                try:
                    value = json.loads(record.read_text())
                    if value['location']['book_id']==books[index][0] and value['location']['sources']:
                        return
                except (OSError,ValueError,KeyError):
                    pass
                time.sleep(.05)
            log.seek(0)
            raise AssertionError(f'{kind} failed to write a valid startup checkpoint: {log.read()[-3000:]}')
        try:
            first, log = start('qt',0); ready(first,log,'qt',0)
            checks.append('Qt acquires its book with malformed legacy startup state')
            blocked, blocked_log = start('tauri',0)
            assert blocked.wait(timeout=25)==2
            blocked_log.seek(0)
            assert 'already open' in blocked_log.read()
            checks.append('Tauri rejects a book already open in Qt')
            second, log = start('tauri',1); ready(second,log,'tauri',1)
            third, log = start('tauri',2); ready(third,log,'tauri',2)
            assert all(p.poll() is None for p in (first,second,third))
            checks.append('Different books run concurrently, including two Tauri processes sharing a scratch root')
            first.kill(); first.wait(timeout=5)
            reopened, log = start('tauri',0); ready(reopened,log,'tauri',0)
            checks.append('A killed Qt process releases its book for Tauri without deleting a lock file')
            report = {'passed':True, 'checks':checks}
        except Exception as error:
            report = {'passed':False, 'checks':checks, 'error':str(error)}
        finally:
            for process,log in processes:
                if process.poll() is None:
                    os.killpg(process.pid,signal.SIGTERM)
                    try: process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid,signal.SIGKILL); process.wait(timeout=5)
                log.close()
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))
        if not report['passed']: raise SystemExit(1)

if __name__=='__main__': main()
