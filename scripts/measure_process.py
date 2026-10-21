#!/usr/bin/env python3
"""Sequential Linux measurements. Standard library only; use synthetic inputs."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import signal
import socket
import subprocess
import sys
import tempfile
import time


def footprint(paths):
    seen = set()
    total = {'apparent_bytes': 0, 'allocated_bytes': 0, 'entries': 0}
    def visit(path):
        st = path.lstat()
        key = (st.st_dev, st.st_ino)
        if key in seen:
            return
        seen.add(key)
        total['entries'] += 1
        total['apparent_bytes'] += st.st_size
        total['allocated_bytes'] += st.st_blocks * 512
        if path.is_dir() and not path.is_symlink():
            for item in path.iterdir():
                visit(item)
    for path in paths:
        visit(Path(path))
    return {'paths': [str(Path(p).resolve()) for p in paths], **total,
            'scope': 'Selected paths only; symlinks not followed; hardlinks counted once; installed dependencies excluded unless explicitly selected.'}


def process_memory(root_pid):
    processes = {}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            processes[int(path.name)] = (int(fields[1]), int(fields[2]))
        except (OSError, ValueError, IndexError):
            continue
    selected = {root_pid} if root_pid in processes else set()
    # Include group members even after their immediate parent has exited.
    selected.update(pid for pid, (_, group) in processes.items() if group == root_pid)
    while True:
        children = {pid for pid, (parent, _) in processes.items() if parent in selected}
        if children <= selected:
            break
        selected |= children
    rss, pss, readable_rss, readable_pss = 0, 0, 0, 0
    for pid in selected:
        try:
            values = Path(f'/proc/{pid}/status').read_text().splitlines()
            rss += int(next(v.split()[1] for v in values if v.startswith('VmRSS:'))) * 1024
            readable_rss += 1
        except (OSError, StopIteration, ValueError):
            pass
        try:
            values = Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines()
            pss += int(next(v.split()[1] for v in values if v.startswith('Pss:'))) * 1024
            readable_pss += 1
        except (OSError, StopIteration, ValueError):
            pass
    return {'process_count': len(selected), 'rss_readable': readable_rss,
            'pss_readable': readable_pss,
            'rss_bytes': rss if selected and readable_rss == len(selected) else None,
            'pss_bytes': pss if selected and readable_pss == len(selected) else None}


def tcp_ready(port):
    try:
        with socket.create_connection(('127.0.0.1', port), timeout=0.05):
            return True
    except OSError:
        return False


def stop(proc):
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    # The parent may have exited while a child ignores SIGTERM.
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    proc.wait()


def run_once(args):
    with tempfile.TemporaryDirectory(prefix='reader-measure-') as tmp:
        ready_path = Path(tmp) / 'ready'
        env = dict(os.environ, READER_MEASURE_READY_FILE=str(ready_path))
        if args.ready_tcp and tcp_ready(args.ready_tcp):
            raise ValueError('Readiness port already accepts connections; stop the existing process first')
        started = time.monotonic()
        proc = subprocess.Popen(args.command, env=env, start_new_session=True,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        samples = []
        ready_at = None
        status = 'timeout'
        try:
            while True:
                now = time.monotonic()
                samples.append({'elapsed_seconds': now - started, **process_memory(proc.pid)})
                if proc.poll() is not None:
                    status = 'exited_before_measurement_complete'
                    break
                ready = tcp_ready(args.ready_tcp) if args.ready_tcp else ready_path.exists()
                if ready_at is None and ready:
                    ready_at = time.monotonic()
                if ready_at is not None and time.monotonic() - ready_at >= args.duration:
                    status = 'ok'
                    break
                if ready_at is None and time.monotonic() - started >= args.timeout:
                    break
                time.sleep(args.interval)
        finally:
            stop(proc)
        def peak(field):
            values = [sample[field] for sample in samples if sample[field] is not None]
            return max(values) if values else None
        return {'status': status, 'startup_seconds': None if ready_at is None else ready_at - started,
                'sampled_peak_rss_bytes': peak('rss_bytes'),
                'sampled_peak_pss_bytes': peak('pss_bytes'),
                'samples': samples}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--label', required=True, help='Include frontend, build mode, revision and fixture')
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--warmups', type=int, default=1)
    parser.add_argument('--duration', type=float, default=5)
    parser.add_argument('--timeout', type=float, default=30)
    parser.add_argument('--interval', type=float, default=0.05)
    parser.add_argument('--ready-tcp', type=int, help='127.0.0.1 only; measures listener readiness, not first paint')
    parser.add_argument('--package', action='append', default=[])
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command and args.command[0] == '--':
        args.command.pop(0)
    if not args.command or args.runs < 1 or args.warmups < 0 or min(args.duration, args.timeout, args.interval) <= 0:
        parser.error('Provide a command, positive timings/runs, and nonnegative warmups')
    if args.ready_tcp is not None and not 1 <= args.ready_tcp <= 65535:
        parser.error('Invalid readiness port')
    if platform.system() != 'Linux':
        parser.error('This measurement tool requires Linux /proc')
    report = {'version': 1, 'label': args.label, 'command': args.command,
              'recorded_at_utc': datetime.now(timezone.utc).isoformat(),
              'host': {'kernel': platform.release(), 'machine': platform.machine(),
                       'python': platform.python_version(), 'logical_cpus': os.cpu_count(),
                       'load_average_at_start': list(os.getloadavg()),
                       'memory_total_kib': int(next(line.split()[1] for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal:'))),
                       'uptime_seconds': float(Path('/proc/uptime').read_text().split()[0])},
              'readiness': 'loopback TCP listener' if args.ready_tcp else 'application readiness file',
              'cache_policy': 'Fresh processes every run. OS filesystem caches are never cleared; cache state is uncontrolled.',
              'interval_seconds': args.interval, 'post_ready_seconds': args.duration,
              'package': footprint(args.package), 'warmups': [], 'runs': []}
    code = 0
    try:
        for index in range(args.warmups + args.runs):
            result = run_once(args)
            result['cache_label'] = 'process-cold/cache-uncontrolled' if index == 0 else 'process-cold/cache-primed'
            target = report['warmups'] if index < args.warmups else report['runs']
            target.append(result)
            if result['status'] != 'ok':
                code = 1
                break
    except (OSError, ValueError) as exc:
        report['error'] = str(exc)
        code = 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(f"{args.output}: {len(report['runs'])} measured runs, {'failed' if code else 'ok'}")
    return code


def demo():
    """Synthetic resident allocation in a parent and a child, no book data."""
    child = subprocess.Popen([sys.executable, '-c', 'import time; data=bytearray(8*1024*1024); time.sleep(120)'])
    data = bytearray(16 * 1024 * 1024)
    time.sleep(0.1)
    Path(os.environ['READER_MEASURE_READY_FILE']).write_text('synthetic ready\n')
    time.sleep(120)
    child.wait()
    return len(data)


if __name__ == '__main__':
    if sys.argv[1:] == ['_demo']:
        demo()
    else:
        raise SystemExit(main())
