#!/usr/bin/env python3
"""Exercise live WebKitGTK theme changes using only a temporary theme tree."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

component = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='theme-', dir=component / '.local') as directory:
    root = Path(directory)
    theme = root / 'current' / 'colors.toml'
    dark = 'mode="dark"\nbackground="#112233"\nforeground="#eeeeee"\naccent="#abcdef"\nselection="#334455"\nselection_foreground="#ffffff"\n'
    light = 'mode="light"\nbackground="#fefefe"\nforeground="#222222"\naccent="#123456"\nselection_background="#ccddee"\nbright_foreground="#111111"\n'
    alternate = 'mode="dark"\nbackground="#223344"\nforeground="#dddddd"\naccent="#fedcba"\nselection="#556677"\nbright_foreground="#eeeeee"\n'
    env = dict(os.environ, READER_THEME_FILE=str(theme), READER_THEME_TEST='1',
               READER_SCRATCH=str(root / 'state'), WEBKIT_DISABLE_DMABUF_RENDERER='1')
    process = subprocess.Popen([str(component / 'src-tauri/target/debug/quiet-reader-tauri'), '--self-test'], env=env)
    try:
        # Start with missing ancestors, then change the same path while the app runs.
        time.sleep(3)
        theme.parent.mkdir()
        theme.write_text(dark)
        time.sleep(3)
        replacement = root / 'replacement'
        replacement.write_text(light)
        replacement.replace(theme)
        time.sleep(3)
        target = root / 'alternate'
        target.mkdir()
        (target / 'colors.toml').write_text(alternate)
        theme.unlink()
        theme.parent.rmdir()
        theme.parent.symlink_to(target, target_is_directory=True)
        time.sleep(3)
        (target / 'colors.toml').unlink()
        time.sleep(3)
        (target / 'colors.toml').write_text(dark)
        code = process.wait(timeout=20)
        result = json.loads((root / 'state/self-test/self-test.json').read_text())
        assert code == 0 and result['passed'], result
        print('Native live theme: missing ancestors, atomic replacement, symlink switch, removal and recreation passed')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
