#!/usr/bin/env python3
"""One local synthetic sample, not a cross-toolkit benchmark."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

component = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else component / 'src-tauri/target/debug/quiet-reader-tauri'
scratch = component / '.local' / f'measure-{time.time_ns()}'
environment = {**os.environ, 'READER_SCRATCH': str(scratch), 'WEBKIT_DISABLE_DMABUF_RENDERER': '1'}
started = time.monotonic()
process = subprocess.Popen([str(binary), '--book-json', str(component / 'web/demo.json')], env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, start_new_session=True)
try:
    while not (scratch / 'state.json').exists():
        if process.poll() is not None:
            raise RuntimeError(f'App exited before readiness with code {process.returncode}')
        if time.monotonic() - started > 30:
            raise TimeoutError('No first state save within 30 seconds')
        time.sleep(.01)
    ready_ms = (time.monotonic() - started) * 1000
    state = json.loads((scratch / 'state.json').read_text())
    saved_book = state['books']['overnight-tauri-synthetic']
    if saved_book['speed'] != 350 or not saved_book['sources']:
        raise RuntimeError('Readiness file did not contain the expected synthetic position and speed')
    time.sleep(5)
    def descendants(pid):
        result = [pid]
        for candidate in Path('/proc').iterdir():
            if not candidate.name.isdigit():
                continue
            try:
                fields = (candidate / 'stat').read_text().split(') ', 1)[1].split()
                if int(fields[1]) == pid:
                    result.extend(descendants(int(candidate.name)))
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                pass
        return result
    samples = []
    for pid in descendants(process.pid):
        try:
            memory = {}
            for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines():
                if line.startswith(('Rss:', 'Pss:')):
                    key, value, _ = line.split()
                    memory[key[:-1].lower() + '_kib'] = int(value)
            samples.append({'pid': pid, 'process': Path(f'/proc/{pid}/comm').read_text().strip(), **memory})
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            samples.append({'pid': pid, 'unavailable': True})
    if process.poll() is not None:
        raise RuntimeError('App exited before memory sampling')
    os.set_blocking(process.stderr.fileno(), False)
    diagnostics = process.stderr.read() or b''
    window = None
    try:
        windows = json.loads(subprocess.check_output(['hyprctl', 'clients', '-j'], text=True))
        own = next((w for w in windows if w.get('pid') == process.pid), None)
        if own:
            window = {key: own.get(key) for key in ['size', 'xwayland']}
    except (FileNotFoundError, subprocess.CalledProcessError):
        pass
    result = {'window': window, 'load_average': os.getloadavg(), 'logical_cpus': os.cpu_count(), 'stderr_bytes_before_shutdown': len(diagnostics), 'verified_synthetic_state': True, 'binary': str(binary.relative_to(component)), 'binary_bytes': binary.stat().st_size, 'readiness_ms': round(ready_ms, 2), 'readiness_definition': 'first persisted state after frontend initial layout, not first paint', 'steady_delay_seconds': 5, 'processes': samples, 'tree_rss_kib': sum(p.get('rss_kib', 0) for p in samples), 'tree_pss_kib': sum(p.get('pss_kib', 0) for p in samples), 'conditions': 'one synthetic sample during concurrent overnight work; no comparative claim'}
    print(json.dumps(result, indent=2))
finally:
    os.killpg(process.pid, signal.SIGTERM)
    process.wait(timeout=10)
