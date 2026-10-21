"""Exercise generated PATH commands in an isolated checkout with fake build tools."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "dev.py"


class Commands(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="reader checkout ' ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.integration = self.root / "integration"
        self.integration.mkdir()
        self.script = self.integration / "dev.py"
        shutil.copyfile(SOURCE, self.script)
        self.bin_dir = self.root / "on path"
        self.bin_dir.mkdir()
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.env = {**os.environ, "PATH": str(self.tools) + os.pathsep + os.environ["PATH"]}
        self.executable(self.tools / "cargo", "import sys\nsys.exit(0)\n")
        for component, name in [("crates/reader-cli", "reader-core"), ("apps/qt", "verse-qt"), ("apps/tauri/src-tauri", "verse-tauri")]:
            (self.root / component).mkdir(parents=True, exist_ok=True)
            target = self.root / "target/debug" / name
            if component == "crates/reader-cli":
                body = '''import json, pathlib, sys
args = sys.argv
pathlib.Path(args[args.index('--output') + 1]).write_text(json.dumps({'epub': args[args.index('--epub') + 1]}))
'''
            else:
                body = '''import json, os, pathlib, sys
args = sys.argv
book = json.loads(pathlib.Path(args[args.index('--book-json')+1]).read_text()) if '--book-json' in args else None
print(json.dumps({'book': book, 'args': args[1:], 'cwd': os.getcwd(), 'qt_state': os.getenv('READER_STATE_DIR'), 'tauri_state': os.getenv('READER_SCRATCH')}))
'''
            self.executable(target, body)

    def executable(self, path, body):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"#!{sys.executable}\n" + body)
        path.chmod(0o755)

    def build(self):
        return subprocess.run([sys.executable, str(self.script), "build", "--debug", "--name", "reader-test", "--bin-dir", str(self.bin_dir)], env=self.env, text=True, capture_output=True)

    def test_commands_open_relative_paths_without_building(self):
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.executable(self.tools / "cargo", "raise SystemExit('launch tried to compile')\n")
        elsewhere = self.root / "elsewhere"
        elsewhere.mkdir()
        epub = elsewhere / "a book's story.epub"
        epub.write_bytes(b"synthetic test input")
        for name, state_key in [("reader-test", "qt_state"), ("reader-test-qt", "qt_state"), ("reader-test-tauri", "tauri_state")]:
            result = subprocess.run([str(self.bin_dir / name), epub.name, "--state-dir", "my state"], cwd=elsewhere, env=self.env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(result.stdout)
            if name.endswith("-tauri"):
                self.assertEqual(value["book"]["epub"], str(epub))
            else:
                self.assertEqual(value["args"], ["--epub", str(epub)])
                self.assertIsNone(value["book"])
            self.assertEqual(value["cwd"], str(elsewhere))
            self.assertEqual(value[state_key], str(elsewhere / "my state"))
        missing = subprocess.run([str(self.bin_dir / "reader-test"), "missing.epub"], cwd=elsewhere, env=self.env, text=True, capture_output=True)
        self.assertNotEqual(missing.returncode, 0)
        self.assertEqual(missing.stdout, "")

    def test_rebuild_refreshes_own_commands_and_preserves_unrelated_files(self):
        self.assertEqual(self.build().returncode, 0)
        self.assertEqual(self.build().returncode, 0)
        command = self.bin_dir / "reader-test"
        command.unlink()
        command.write_text("unrelated command")
        result = self.build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Refusing to replace", result.stderr)
        self.assertEqual(command.read_text(), "unrelated command")


if __name__ == "__main__":
    unittest.main()
