"""Exercise command boundaries with explicit fake drivers, never product evidence."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from fixtures import RESTORE
from validate import run_core

ROOT=Path(__file__).resolve().parents[1]


class CommandTests(unittest.TestCase):
    def test_export_runner_continues_and_rejects_valid_input_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            driver=root/'reject.py'
            driver.write_text('import sys\nprint("Synthetic deliberate rejection",file=sys.stderr)\nsys.exit(2)\n')
            with contextlib.redirect_stdout(io.StringIO()):
                status=run_core([sys.executable,str(driver)],root/'out',5)
            self.assertEqual(status,1)
            report=json.loads((root/'out/results.json').read_text())
            self.assertEqual(len(report['results']),11)
            self.assertEqual(sum(r['status']=='pass' for r in report['results']),3)
            self.assertEqual(sum(r['status']=='fail' for r in report['results']),8)

    def test_failed_import_must_not_leave_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            driver=root/'partial.py'
            driver.write_text('import pathlib,sys\npathlib.Path(sys.argv[-1]).write_text("{}")\nprint("Synthetic rejection",file=sys.stderr)\nsys.exit(2)\n')
            with contextlib.redirect_stdout(io.StringIO()):
                status=run_core([sys.executable,str(driver)],root/'out',5)
            self.assertEqual(status,1)
            report=json.loads((root/'out/results.json').read_text())
            self.assertTrue(all(r['status']=='fail' for r in report['results']))

    def test_restore_runner_withholds_expected_answers(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            driver=root/'restore.py'
            results=[dict(c['expected'],id=c['id']) for c in RESTORE]
            driver.write_text('import json,sys\nrequests=json.load(sys.stdin)\nassert all("expected" not in c for c in requests)\nprint('+repr(json.dumps(results))+')\n')
            completed=subprocess.run([sys.executable,str(ROOT/'validate.py'),'run-restore','--output',str(root/'actual.json'),'--',sys.executable,str(driver)],capture_output=True,text=True)
            self.assertEqual(completed.returncode,0,completed.stderr)
            self.assertEqual(json.loads((root/'actual.json').read_text()),results)

    def test_cli_reports_invalid_json_without_traceback(self):
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'bad.json'; source.write_text('[]')
            completed=subprocess.run([sys.executable,str(ROOT/'validate.py'),'validate','--case','logical_chapters','--book-json',str(source)],capture_output=True,text=True)
            self.assertEqual(completed.returncode,1)
            self.assertIn('FAIL:',completed.stderr)
            self.assertNotIn('Traceback',completed.stderr)


if __name__=='__main__':unittest.main()
