#!/usr/bin/env python3
"""Measure a synthetic Qt reader session; all reported sizes are bytes."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def process_tree(root_pid):
    parents = {}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            status = (entry / 'status').read_text()
            parent = re.search(r'^PPid:\s+(\d+)$', status, re.MULTILINE)
            if parent:
                parents[int(entry.name)] = int(parent.group(1))
        except (OSError, ValueError):
            continue
    found = {root_pid}
    while True:
        expanded = found | {pid for pid, parent in parents.items() if parent in found}
        if expanded == found:
            return sorted(found)
        found = expanded


def sample(root_pid):
    processes = []
    libraries = {}
    errors = []
    for pid in process_tree(root_pid):
        item = {'pid': pid}
        try:
            rollup = Path(f'/proc/{pid}/smaps_rollup').read_text()
            for label in ('Rss', 'Pss'):
                value = re.search(rf'^{label}:\s+(\d+) kB$', rollup, re.MULTILINE)
                if not value:
                    raise ValueError(f'missing {label}')
                item[label.lower() + '_bytes'] = int(value.group(1)) * 1024
        except (OSError, ValueError) as error:
            errors.append(f'pid {pid} memory unavailable: {error}')
        try:
            for line in Path(f'/proc/{pid}/maps').read_text().splitlines():
                columns = line.split(None, 5)
                if len(columns) < 6 or not columns[5].startswith('/'):
                    continue
                path = Path(columns[5])
                if '.so' not in path.name:
                    continue
                try:
                    resolved = path.resolve(strict=True)
                    libraries[str(resolved)] = resolved.stat().st_size
                except OSError as error:
                    errors.append(f'library stat unavailable: {error}')
        except OSError as error:
            errors.append(f'pid {pid} maps unavailable: {error}')
        processes.append(item)
    return {
        'processes': processes,
        'process_tree_rss_bytes': sum(p.get('rss_bytes', 0) for p in processes),
        'process_tree_pss_bytes': sum(p.get('pss_bytes', 0) for p in processes),
        'memory_complete': all('pss_bytes' in p and 'rss_bytes' in p for p in processes),
        'mapped_shared_libraries': [dict(path=p, file_bytes=s) for p, s in sorted(libraries.items())],
        'mapped_shared_library_file_bytes': sum(libraries.values()),
        'errors': errors,
    }


def trial(binary, sample_seconds):
    with tempfile.TemporaryDirectory(prefix='measure-', dir=ROOT / '.local') as directory:
        directory = Path(directory)
        env = os.environ.copy()
        env.update({
            'READER_STATE_DIR': str(directory / 'state'),
            'READER_THEME_FILE': str(ROOT / 'fixtures/theme-light.toml'),
            'QT_QPA_PLATFORM': 'offscreen',
            'QT_QUICK_BACKEND': 'software',
            'QT_SCALE_FACTOR': '1',
            'QT_FORCE_STDERR_LOGGING': '1',
            'READER_MEASURE': '1',
        })
        with (directory / 'stderr.log').open('w+b') as stderr:
            started = time.monotonic()
            process = subprocess.Popen(
                [str(binary), '--book-json', str(ROOT / 'fixtures/demo.json')],
                cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=stderr,
                start_new_session=True,
            )
            result = {}
            try:
                deadline = started + sample_seconds
                while time.monotonic() < deadline and process.poll() is None:
                    time.sleep(min(0.02, max(0, deadline - time.monotonic())))
                result['process_spawn_to_sample_ms'] = round((time.monotonic() - started) * 1000, 3)
                result['running_at_sample'] = process.poll() is None
                if result['running_at_sample']:
                    result.update(sample(process.pid))
                else:
                    result['errors'] = ['Application exited before memory sample']
            finally:
                # No application persistence is requested for measurement trials.
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    result.setdefault('errors', []).append('SIGKILL needed after SIGTERM timeout')
            stderr.seek(0)
            diagnostic = stderr.read()
            platforms = re.findall(rb'READER_PLATFORM=([^\r\n]+)', diagnostic)
            result['qt_platform'] = platforms[0].decode() if platforms else None
            markers = re.findall(rb'READER_FIRST_FRAME_MS=(\d+(?:\.\d+)?)', diagnostic)
            result['native_first_frame_ms'] = float(markers[0]) if markers else None
            result['first_frame_marker_observed'] = bool(markers)
            result['exit_code_after_termination'] = process.returncode
            return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/overnight-qt-reader')
    parser.add_argument('--trials', type=int, default=3)
    parser.add_argument('--sample-seconds', type=float, default=1.0)
    args = parser.parse_args()
    if args.trials < 1 or args.sample_seconds <= 0:
        raise ValueError('trials and sample-seconds must be positive')
    binary = args.binary.resolve(strict=True)
    (ROOT / '.local').mkdir(exist_ok=True)
    report = {
        'binary': str(binary),
        'executable_file_bytes': binary.stat().st_size,
        'fixture': 'fixtures/demo.json',
        'rendering': 'Qt offscreen platform, software Qt Quick backend, QT_SCALE_FACTOR=1',
        'sample_delay_seconds': args.sample_seconds,
        'inherited_qt_platform_theme': os.environ.get('QT_QPA_PLATFORMTHEME'),
        'inherited_qt_style_override': os.environ.get('QT_STYLE_OVERRIDE'),
        'notes': [
            'RSS summed across processes can count shared pages more than once; PSS apportions shared pages.',
            'Shared library file sizes are unique mapped paths, separate from executable size; they are not bundle size or memory.',
            'native_first_frame_ms measures native entry before QGuiApplication to first frameSwapped, not external process startup; null means no startup evidence.',
            'Trials run sequentially on the current host; concurrent unrelated workloads are uncontrolled.',
        ],
        'trials': [trial(binary, args.sample_seconds) for _ in range(args.trials)],
    }
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'error': str(error)}))
        raise SystemExit(1)
