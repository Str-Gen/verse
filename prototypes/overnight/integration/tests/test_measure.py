import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest

MODULE = Path(__file__).resolve().parents[1] / 'measure_process.py'
spec = importlib.util.spec_from_file_location('measure', MODULE)
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)


class MeasurementTests(unittest.TestCase):
    def test_footprint_counts_hardlinks_once_without_following_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / 'file'
            file.write_bytes(b'abc')
            linked = root / 'linked'
            os.link(file, linked)
            symlink = root / 'symlink'
            symlink.symlink_to(file)
            result = measure.footprint([str(file), str(linked), str(symlink)])
            self.assertEqual(result['entries'], 2)
            self.assertEqual(result['apparent_bytes'], 3 + symlink.lstat().st_size)

    def args(self, command):
        return SimpleNamespace(command=command, ready_tcp=None, duration=0.15,
                               timeout=0.3, interval=0.02)

    def test_synthetic_process_tree_is_measured(self):
        result = measure.run_once(self.args([sys.executable, str(MODULE), '_demo']))
        self.assertEqual(result['status'], 'ok')
        self.assertIsNotNone(result['startup_seconds'])
        self.assertTrue(any(s['process_count'] >= 2 for s in result['samples']))
        self.assertGreater(result['sampled_peak_rss_bytes'], 24 * 1024 * 1024)

    def test_missing_readiness_times_out(self):
        result = measure.run_once(self.args([sys.executable, '-c', 'import time; time.sleep(10)']))
        self.assertEqual(result['status'], 'timeout')
        self.assertIsNone(result['startup_seconds'])

    def test_timeout_terminates_spawned_descendants(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file = Path(directory) / 'child-pid'
            command = [sys.executable, '-c',
                       'import subprocess, sys, time; from pathlib import Path; '
                       'p=subprocess.Popen([sys.executable, "-c", "import time; time.sleep(120)"]); '
                       'Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(120)', str(pid_file)]
            result = measure.run_once(self.args(command))
            self.assertEqual(result['status'], 'timeout')
            child_pid = int(pid_file.read_text())
            stat = Path(f'/proc/{child_pid}/stat')
            # A terminated orphan can remain a zombie until init reaps it.
            deadline = time.monotonic() + 1
            while stat.exists():
                try:
                    state = stat.read_text().rsplit(')', 1)[1].split()[0]
                except FileNotFoundError:
                    break
                if state == 'Z':
                    break
                self.assertLess(time.monotonic(), deadline, 'spawned child survived cleanup')
                time.sleep(0.01)

    def test_early_exit_is_failure(self):
        result = measure.run_once(self.args([sys.executable, '-c', 'pass']))
        self.assertEqual(result['status'], 'exited_before_measurement_complete')


if __name__ == '__main__':
    unittest.main()
