#!/usr/bin/env python3
"""Run the Tauri native suite across intact and damaged saved positions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    book = json.loads(args.fixture.read_text())
    key = hashlib.sha256(book["book_id"].encode()).hexdigest()
    reports = []
    with tempfile.TemporaryDirectory(prefix="reader-recovery-") as temporary:
        scratch = Path(temporary)
        primary = scratch / "self-test/state.books" / (key + ".json")
        backup = primary.with_name(key + ".backup.json")
        environment = {**os.environ, "READER_SCRATCH":str(scratch),
                       "WEBKIT_DISABLE_DMABUF_RENDERER":"1", "TYPESAFE_API_KEY":"",
                       "VERSE_KEY_FILE":str(scratch / "absent")}
        for damage in ("intact", "truncated", "invalid-source"):
            if damage != "intact":
                shutil.copyfile(primary, backup)
                if damage == "truncated":
                    primary.write_text("{")
                else:
                    record = json.loads(primary.read_text())
                    record["location"]["sources"][0]["node"] = "missing-node"
                    record["location"]["context"] = "not a real passage"
                    primary.write_text(json.dumps(record))
            result = subprocess.run([str(binary), "--self-test"], env=environment,
                                    capture_output=True, text=True, timeout=120)
            report = json.loads((scratch / "self-test/self-test.json").read_text())
            reports.append({"damage":damage, "exit_code":result.returncode,
                            "stderr":result.stderr, "result":report})
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps({"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
                                              "runs":reports}, indent=2) + "\n")
            assert result.returncode == 0 and report["passed"], reports[-1]
            print(damage, len(report["checks"]), "checks passed", flush=True)


if __name__ == "__main__":
    main()
