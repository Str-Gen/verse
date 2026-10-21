#!/usr/bin/env python3
"""Measure native text-memory scaling; run only against an already-built binary.

Example:
  python3 memory.py --frontend tauri --binary /path/to/verse-tauri \
      --output /path/to/memory-tauri.json

Five runs per size alternate small/large fixtures. The frontend announces reading
only after startup/spooling, then must complete at least 40 distant seeks. Linux
PSS is sampled for the host and recursively discovered children. Sampling cannot
observe every allocation peak; this is a scaling measurement, not a speed claim
or a comparison with an old implementation.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import tempfile
import time
import uuid


WORDS = "The quiet reader follows each sentence across the page and returns to a familiar passage".split()


def emit(writer, value):
    json.dump(value, writer, ensure_ascii=False, separators=(",", ":"))


def fixture(path: Path, count: int):
    """Keep at most one 128-word paragraph in the fixture generator's memory."""
    identity = f"memory-synthetic-{count}-{uuid.uuid4()}"
    document = "synthetic.xhtml"
    with path.open("w", encoding="utf-8") as writer:
        header = {"schema_version": 1, "book_id": identity, "revision": f"synthetic-{count}",
                  "title": f"Synthetic memory fixture ({count} pieces)",
                  "publisher_identifiers": [], "resources": [], "image_evidence": [], "diagnostics": [], "notes": []}
        writer.write("{")
        for index, (key, value) in enumerate(header.items()):
            if index:
                writer.write(",")
            emit(writer, key)
            writer.write(":")
            emit(writer, value)
        writer.write(',"documents":[{"id":"synthetic.xhtml","blocks":[')
        for start in range(0, count, 128):
            if start:
                writer.write(",")
            text = " ".join(WORDS[index % len(WORDS)] for index in range(start, min(count, start + 128)))
            source = {"document": document, "node": f"0/{start // 128}/text", "offset": 0, "length": len(text)}
            emit(writer, {"id": f"b{start // 128}", "kind": "paragraph", "runs": [{"text": text, "sources": [source]}]})
        writer.write(']}],"pieces":[')
        offset = 0
        for index in range(count):
            if index:
                writer.write(",")
            if index % 128 == 0:
                offset = 0
            word = WORDS[index % len(WORDS)]
            emit(writer, {"id": f"p{index}", "word_id": f"w{index}", "text": word,
                          "kind": "word", "block_id": f"b{index // 128}",
                          "paragraph_end": index % 128 == 127 or index == count - 1,
                          "sources": [{"document": document, "node": f"0/{index // 128}/text", "offset": offset, "length": len(word)}]})
            offset += len(word) + 1
        writer.write('],"chapters":[')
        for start in range(0, count, 128):
            if start:
                writer.write(",")
            emit(writer, {"id": f"chapter-{start // 128}", "title": f"Chapter {start // 128 + 1}",
                          "start_piece": f"p{start}", "end_piece_exclusive": f"p{start + 128}" if start + 128 < count else None})
        writer.write("]}\n")
    return {"pieces": count, "bytes": path.stat().st_size, "sha256": fingerprint(path), "book_id": identity}


def fingerprint(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return None


def process_tree(root):
    children = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            fields = (entry / "stat").read_text().rsplit(") ", 1)[1].split()
            children.setdefault(int(fields[1]), []).append(int(entry.name))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    pending, found = [root], set()
    while pending:
        pid = pending.pop()
        if pid in found:
            continue
        found.add(pid)
        pending.extend(children.get(pid, []))
    return found


def sample(root):
    values, unavailable = {}, []
    for pid in process_tree(root):
        try:
            fields = {}
            for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
                if line.startswith(("Pss:", "Rss:")):
                    key, value, _unit = line.split()
                    fields[key[:-1].lower()] = int(value)
            fields["threads"] = int(next(line.split()[1] for line in Path(f"/proc/{pid}/status").read_text().splitlines() if line.startswith("Threads:")))
            fields["name"] = Path(f"/proc/{pid}/comm").read_text().strip()
            if "pss" in fields and "rss" in fields:
                values[pid] = fields
            else:
                unavailable.append(pid)
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            unavailable.append(pid)
    if root not in values:
        return None
    host = values[root]
    children = [value for pid, value in values.items() if pid != root]
    children_pss = sum(value["pss"] for value in children)
    return {"host_pss_kib": host["pss"], "children_pss_kib": children_pss,
            "tree_pss_kib": host["pss"] + children_pss,
            "tree_rss_kib": sum(value["rss"] for value in values.values()),
            "measured_processes": len(values), "tree_threads": sum(value["threads"] for value in values.values()),
            "process_names": sorted({value["name"] for value in values.values()}), "unavailable_pids": unavailable}


def peaks(samples):
    if not samples:
        return None
    high = max(samples, key=lambda row: row["tree_pss_kib"])
    return {"samples": len(samples),
            "tree_peak_pss_kib": high["tree_pss_kib"],
            "host_at_tree_peak_pss_kib": high["host_pss_kib"],
            "children_at_tree_peak_pss_kib": high["children_pss_kib"],
            "host_peak_pss_kib": max(row["host_pss_kib"] for row in samples),
            "children_peak_pss_kib": max(row["children_pss_kib"] for row in samples),
            "max_tree_threads": max(row["tree_threads"] for row in samples),
            "incomplete_samples": sum(bool(row["unavailable_pids"]) for row in samples)}


def work_count(result):
    work = result.get("work", result.get("windows", 0))
    if isinstance(work, dict):
        work = work.get("seeks", work.get("completed_seeks", work.get("windows", 0)))
    return work if isinstance(work, int) and not isinstance(work, bool) else 0


def terminate(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
    # A failed host can leave helper processes in its process group.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass


def run_once(args, book, root, index, size):
    scratch = root / f"run-{index}-{size}"
    scratch.mkdir()
    environment = {key: value for key, value in os.environ.items() if not key.endswith("API_KEY")}
    environment.update(READER_MEMORY_TEST="1", READER_SCRATCH=str(scratch), READER_STATE_DIR=str(scratch),
                       WEBKIT_DISABLE_DMABUF_RENDERER="1")
    command = [str(args.binary), "--book-json", str(book)]
    if args.frontend == "tauri":
        command.append("--self-test")
    else:
        environment.update(READER_SMOKE="memory", QT_QPA_PLATFORM=args.qt_platform, QT_QUICK_BACKEND=args.qt_backend)
    phase_path = scratch / "self-test/memory-phase.json"
    result_path = scratch / "self-test/self-test.json"
    rows, reading_started, result = [], None, None
    started = time.monotonic()
    log_path = scratch / "native.log"
    timed_out = False
    with log_path.open("wb") as log:
        process = subprocess.Popen(command, env=environment, stdout=log, stderr=log, start_new_session=True)
        try:
            while process.poll() is None:
                now = time.monotonic()
                if now - started > args.timeout:
                    timed_out = True
                    break
                phase = read_json(phase_path)
                if isinstance(phase, dict) and phase.get("phase") == "reading" and reading_started is None:
                    reading_started = now - started
                row = sample(process.pid)
                if row is not None:
                    rows.append({"seconds": round(now - started, 4), "phase": "reading" if reading_started is not None else "startup", **row})
                time.sleep(args.interval)
            exit_code = process.poll()
            result = read_json(result_path)
        finally:
            terminate(process)
    completed = work_count(result) if isinstance(result, dict) else 0
    reading = [row for row in rows if row["phase"] == "reading"]
    passed = (not timed_out and exit_code == 0 and isinstance(result, dict)
              and result.get("passed") is True and completed >= 40 and bool(reading))
    with log_path.open("rb") as log:
        log.seek(max(0, log_path.stat().st_size - 6000))
        tail = log.read().decode("utf-8", errors="replace")
    return {"attempt": index, "size": size, "passed": passed, "exit_code": exit_code, "command": command,
            "timed_out": timed_out, "completed_seeks": completed, "frontend_result": result,
            "reading_started_seconds": reading_started, "elapsed_seconds": round(time.monotonic() - started, 3),
            "lifecycle": peaks(rows), "reading": peaks(reading), "samples": rows,
            "environment": {key: environment.get(key) for key in ["DISPLAY", "WAYLAND_DISPLAY", "GDK_BACKEND", "QT_QPA_PLATFORM", "QT_QUICK_BACKEND", "WEBKIT_DISABLE_DMABUF_RENDERER"]},
            "load_average": os.getloadavg(), "log_tail": tail}


def summarize(rows):
    summary = {}
    for size in ("small", "large"):
        completed = [row for row in rows if row["size"] == size and row["passed"]]
        metrics = {}
        for phase in ("lifecycle", "reading"):
            metrics[phase] = {}
            for key in ("tree_peak_pss_kib", "host_peak_pss_kib", "children_peak_pss_kib"):
                values = [row[phase][key] for row in completed]
                metrics[phase][key] = {"median": statistics.median(values), "min": min(values), "max": max(values)} if values else None
        summary[size] = {"successful_runs": len(completed), **metrics}
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--frontend", choices=("tauri", "qt"), default="tauri")
    parser.add_argument("--output", "--artifact", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5, help="runs per size; default ten total launches")
    parser.add_argument("--small-pieces", type=int, default=5000)
    parser.add_argument("--large-pieces", type=int, default=50000)
    parser.add_argument("--interval", type=float, default=.05)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--qt-platform", default="offscreen")
    parser.add_argument("--qt-backend", default="software")
    args = parser.parse_args()
    if args.runs < 1 or args.small_pieces < 256 or args.large_pieces <= args.small_pieces or args.interval <= 0 or args.timeout <= 0:
        parser.error("require positive runs/timings and 256 <= small-pieces < large-pieces")
    args.binary = args.binary.resolve(strict=True)
    args.output = args.output.resolve()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"timestamp": datetime.now(timezone.utc).isoformat(), "frontend": args.frontend,
              "binary": str(args.binary), "binary_sha256": fingerprint(args.binary),
              "binary_bytes": args.binary.stat().st_size, "runs_per_size": args.runs,
              "sample_interval_seconds": args.interval, "results": [],
              "conditions": ["Same provided binary for both fixture sizes; no builds or compiler flags changed.",
                  "Text-only synthetic fixtures; images and large footnotes are not measured.",
                  "Private fresh reading state per run; OS/filesystem cache is uncontrolled and warms over alternating runs.",
                  "Reading starts after the frontend reports startup/spooling complete; at least 40 distant seeks must pass.",
                  "PSS apportions shared pages across processes; recursive children are included. Sampled peaks can miss brief allocation peaks.",
                  "Qt defaults to offscreen/software rendering; the actual environment is recorded. Do not infer a toolkit comparison.",
                  "No historical baseline or speedup is claimed. Median/range describe observed scaling in this environment."]}
    with tempfile.TemporaryDirectory(prefix="native-memory-", dir=args.output.parent) as directory:
        root = Path(directory)
        paths = {"small": root / "small.json", "large": root / "large.json"}
        report["fixtures"] = {size: fixture(paths[size], count) for size, count in (("small", args.small_pieces), ("large", args.large_pieces))}
        for attempt in range(args.runs):
            for size in ("small", "large"):
                if fingerprint(args.binary) != report["binary_sha256"]:
                    raise SystemExit("Binary changed during measurement; previous runs retained in artifact")
                row = run_once(args, paths[size], root, attempt, size)
                if fingerprint(args.binary) != report["binary_sha256"]:
                    row["passed"] = False
                    row["binary_changed_during_run"] = True
                report["results"].append(row)
                report["summary"] = summarize(report["results"])
                args.output.write_text(json.dumps(report, indent=2) + "\n")
                print(f"{args.frontend} {size} run {attempt + 1}/{args.runs}: {'PASS' if row['passed'] else 'FAIL'}; reading={row['reading']}", flush=True)
                if not row["passed"]:
                    raise SystemExit(f"Native memory protocol failed; details saved to {args.output}")
    print(json.dumps(report["summary"], indent=2))
    print(f"Artifact: {args.output}")


if __name__ == "__main__":
    main()
