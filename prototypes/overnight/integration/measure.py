#!/usr/bin/env python3
"""Sequential release process-tree samples on one synthetic core export."""
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('independent_measure', HERE/'../validation/measure.py')
measurement = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measurement)


def builders():
    return [line.strip() for line in subprocess.check_output(['ps','-eo','pid,comm'],text=True).splitlines()[1:]
            if line.split()[-1] in ('cargo','cargo-clippy','rustc','cc1plus','cc1','ninja','cmake','make')]


def main():
    assert not builders(), ('Builds still running', builders())
    book = HERE/'evidence/core-demo.json'
    data = json.loads(book.read_text())
    results = []
    for frontend, binary in [('qt',HERE/'../qt/target/release/overnight-qt-reader'),('tauri',HERE/'../tauri/src-tauri/target/release/quiet-reader-tauri')]:
        binary = binary.resolve(strict=True)
        dependencies = subprocess.check_output(['ldd',str(binary)],text=True)
        paths = set()
        for line in dependencies.splitlines():
            for word in line.split():
                if word.startswith('/') and Path(word).is_file(): paths.add(Path(word).resolve())
        for attempt in range(3):
            assert not builders(), builders()
            state = HERE/'.local'/f'measure-{frontend}-{time.time_ns()}'
            state.mkdir(parents=True)
            environment = dict(os.environ, READER_STATE_DIR=str(state), READER_SCRATCH=str(state),
                READER_THEME_FILE=str(HERE/'../qt/fixtures/theme-light.toml'), QT_QPA_PLATFORM='wayland',
                QT_FORCE_STDERR_LOGGING='1', QT_SCALE_FACTOR='1', READER_MEASURE='1', WEBKIT_DISABLE_DMABUF_RENDERER='1')
            log = HERE/'evidence'/f'measure-{frontend}-{attempt}.log'
            with log.open('wb') as stderr:
                started = time.monotonic()
                process = subprocess.Popen([str(binary),'--book-json',str(book)],env=environment,stdout=stderr,stderr=stderr,start_new_session=True)
                try:
                    window = None
                    while time.monotonic()-started < 30:
                        assert process.poll() is None, 'Application exited before mapped window'
                        clients = json.loads(subprocess.check_output(['hyprctl','clients','-j'],text=True))
                        window = next((w for w in clients if w['pid']==process.pid and w.get('mapped')),None)
                        if window: break
                        time.sleep(.02)
                    assert window, 'No mapped native window'
                    mapped_ms = (time.monotonic()-started)*1000
                    time.sleep(5)
                    assert process.poll() is None
                    memory = measurement.process_memory(process.pid)
                    clients = json.loads(subprocess.check_output(['hyprctl','clients','-j'],text=True))
                    window = next(w for w in clients if w['pid']==process.pid)
                    if frontend == 'tauri':
                        saved = json.loads((state/'state.json').read_text())['books'][data['book_id']]
                        assert saved['speed']==350 and saved['sources']
                    else:
                        assert Path((state/'last-book').read_text()).resolve()==book.resolve()
                    capture = None
                    if attempt == 0:
                        x,y = window['at']; width,height = window['size']
                        capture = HERE/'evidence'/f'{frontend}-release-native.png'
                        subprocess.run(['grim','-g',f'{x},{y} {width}x{height}',str(capture)],check=True)
                    row = dict(frontend=frontend,attempt=attempt,cache='uncontrolled first process' if attempt==0 else 'primed filesystem cache, fresh storage/process',
                        binary=str(binary),executable_bytes=binary.stat().st_size,executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                        installed_shared_library_bytes=sum(p.stat().st_size for p in paths),installed_shared_library_files=len(paths),
                        mapped_window_ms=mapped_ms,window={key:window.get(key) for key in ('size','xwayland')},
                        load_average=os.getloadavg(),build_processes=builders(),memory=memory,
                        capture=str(capture) if capture else None,log=str(log),storage=str(state),
                        contaminated_by_other_activity=True)
                    results.append(row)
                finally:
                    measurement.stop(process)
            time.sleep(.5)
    report = dict(timestamp=datetime.now(timezone.utc).isoformat(),book_sha256=hashlib.sha256(book.read_bytes()).hexdigest(),results=results,
        conditions=['All four workers had completed; no compiler/build processes observed before each launch or at sampling.',
                    'Sequential native Wayland release processes; default component release profiles differ. Qt does not use Tauri thin LTO/stripping settings.',
                    'Startup measures compositor mapped-window observation, not first content/paint. Polling and hyprctl add overhead.',
                    'Memory is a sample after five seconds, not peak usage. RSS double counts sharing; PSS apportions it.',
                    'Contaminated by uncontrolled desktop/background activity. CPU load is recorded; no causal performance or production footprint claim.',
                    'Executable and installed shared-library totals exclude helper executables, plugins and package assets; no installer bundle.'])
    (HERE/'evidence/measurements.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))

if __name__ == '__main__': main()
