#!/usr/bin/env python3
"""Measure book-ready startup with cold and reused application caches, sequentially."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]


def launch(binary, book, directory, cache, imports, platform, saved_state, legacy=False):
    state = directory / 'state'
    if saved_state:
        shutil.copytree(saved_state, state)
    env = dict(os.environ, QT_QPA_PLATFORM=platform, QT_QUICK_BACKEND='software',
               QT_SCALE_FACTOR='1', QT_FORCE_STDERR_LOGGING='1', READER_MEASURE='1', XDG_CACHE_HOME=str(cache),
               READER_IMPORT_DIR=str(imports),
               READER_THEME_FILE=str(ROOT / 'apps/qt/fixtures/theme-light.toml'))
    flag = '--epub' if book.suffix.lower() == '.epub' else '--book-json'
    marker = 'READER_FIRST_FRAME_MS' if legacy else 'READER_BOOK_READY_MS'
    with (directory / 'stderr.log').open('w+b') as log:
        start = time.monotonic()
        process = subprocess.Popen([str(binary), flag, str(book), '--state-dir', str(state)],
                                   env=env, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=log,
                                   start_new_session=True)
        try:
            while time.monotonic() - start < 30:
                log.seek(0)
                diagnostic = log.read().decode(errors='replace')
                ready = re.search(rf'{marker}=(\d+)', diagnostic)
                if ready:
                    remembered = json.loads((state / 'last-book.json').read_text())
                    if Path(remembered).resolve() != book:
                        raise RuntimeError('Requested book was not opened')
                    frame = re.search(r'READER_FIRST_FRAME_MS=(\d+)', diagnostic)
                    row = {'ready_ms': int(ready[1]), 'ready_marker': marker,
                           'first_frame_ms': int(frame[1]) if frame else None,
                           'spawn_to_ready_ms': round((time.monotonic() - start) * 1000, 2)}
                    stat = Path(f'/proc/{process.pid}/stat').read_text().split()
                    tick_ms = 1000 / os.sysconf('SC_CLK_TCK')
                    row['user_cpu_ms'] = round(int(stat[13]) * tick_ms, 2)
                    row['system_cpu_ms'] = round(int(stat[14]) * tick_ms, 2)
                    for proc_file in ('io', 'smaps_rollup'):
                        row[proc_file] = Path(f'/proc/{process.pid}/{proc_file}').read_text()
                    return row
                if process.poll() is not None:
                    raise RuntimeError(f'Application exited before readiness: {diagnostic}')
                time.sleep(0.005)
            raise RuntimeError(f'Book-ready timeout; opened state exists={(state / "last-book.json").exists()}: {diagnostic}')
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('book', type=Path)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/verse-qt')
    parser.add_argument('--baseline', type=Path, help='Previous synchronous binary, measured at its first book frame')
    parser.add_argument('--state', type=Path, help='Copy this state for saved-position trials; never writes to it')
    parser.add_argument('--trials', type=int, default=5)
    parser.add_argument('--platform', choices=['offscreen', 'wayland'], default='offscreen')
    parser.add_argument('--max-warm-ms', type=float)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.trials < 1:
        parser.error('--trials must be positive')
    args.book = args.book.resolve(strict=True)
    args.binary = args.binary.resolve(strict=True)
    if args.baseline:
        args.baseline = args.baseline.resolve(strict=True)
    scratch = ROOT / 'apps/qt/.local'
    scratch.mkdir(exist_ok=True)
    rows = []
    for trial in range(args.trials):
        # Keep caches on the checkout filesystem, not /tmp, which may be tmpfs.
        with tempfile.TemporaryDirectory(prefix='startup-', dir=scratch) as temporary:
            directory = Path(temporary)
            imports = directory / 'imports'
            identities = ROOT / 'scripts/.local/identities'
            if identities.exists():
                shutil.copytree(identities, imports / 'identities')
            scenarios = [('cold', args.binary), ('warm', args.binary)]
            if args.baseline:
                scenarios.insert(0, ('baseline', args.baseline))
            for scenario, binary in scenarios:
                case = directory / scenario
                case.mkdir()
                cache = directory / ('baseline-cache' if scenario == 'baseline' else 'cache')
                row = launch(binary, args.book, case, cache, imports, args.platform,
                             args.state, legacy=scenario == 'baseline')
                rows.append(dict(trial=trial, scenario=scenario, **row))
    summary = {}
    for name in sorted({row['scenario'] for row in rows}):
        values = [row['ready_ms'] for row in rows if row['scenario'] == name]
        summary[name] = dict(median_ms=statistics.median(values), min_ms=min(values), max_ms=max(values))
    report = dict(book=str(args.book), binary=str(args.binary), platform=args.platform,
                  notes=['OS page caches are not flushed. Cold/warm describe application caches.',
                         'After readiness includes initial pagination and a presented frame.',
                         'Legacy baseline ends at its first frame after synchronous loading, before the delayed pagination timer.',
                         'Memory and I/O counters cover the main process; other host workloads are uncontrolled.'],
                  summary=summary, trials=rows)
    encoded = json.dumps(report, indent=2) + '\n'
    if args.output:
        args.output.write_text(encoded)
    print(json.dumps(summary, indent=2))
    if args.max_warm_ms is not None and summary['warm']['max_ms'] > args.max_warm_ms:
        raise SystemExit('Warm startup exceeded the requested limit')


if __name__ == '__main__':
    main()
