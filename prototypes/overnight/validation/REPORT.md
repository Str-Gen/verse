# Worker 4 validation report

Completed the independent validation tools in `prototypes/overnight/validation`. The core and native frontend comparisons are pending integration. This report does not claim either frontend or the EPUB importer passed.

The one-command local launch is:

```sh
./prototypes/overnight/validation/run.sh
```

It needs Python 3.12+ and Linux for process measurements. It runs the checker tests, generates synthetic EPUBs and a schema-v1 pagination book, writes the native UI action plan, and measures a synthetic parent/child process. No dependencies were downloaded. No Rust or TypeScript implementation was added, and no shared manifests or build directories were used.

## Delivered

- Eleven reproducible EPUB inputs, including invalid-container and missing-resource cases. Hand-authored expectations cover joined inline words, scalar source spans, astral Unicode and combining marks, hidden text, fragment chapters, chapters spanning files, front matter, notes, ornaments, repeated substantive images, unsupported content and edited editions.
- An executable core export runner and checker. It checks exact words and spans, conventional/playback agreement, event order and sources, note links, logical chapter ranges, failure diagnostics, distinct revision identities and stable revision identity on repeat export.
- Six independent restore-policy examples and a subprocess adapter runner that withholds expected answers. Candidate discovery and confidence remain separate integration work.
- A long, original synthetic book, native UI action protocol and trace checker for page arrows, exact passage selection/start/return, reflow, stopped restart, saved speed and scalar/UTF-16 conversion.
- A Linux startup, process-tree RSS/PSS and selected-package footprint tool with sequential warmups/runs, explicit cache labels, readiness timeout and process-group cleanup. A committed-snapshot helper keeps sibling mutable source/builds out of local verification.
- Usage, adapter requirements and comparison procedure in `README.md` and `MEASUREMENTS.md`.

## Commands and observed results

Executed from this worktree's repository root on 2026-10-21, Europe/Brussels:

```sh
./prototypes/overnight/validation/run.sh
```

Exit 0. All 18 tests passed. Generated 11 synthetic EPUBs, one two-chapter/80-paragraph JSON book and one UI plan. The measurement demonstration completed one warmup and two measured runs, each with status `ok`. Outputs are in this component's ignored `.scratch/` directory. The tests include deliberate bad source offsets, missing emphasis, wrong chapter ranges, invalid note links, skipped pages, dropped words, browser-only provenance, bad persistence observations, early process exit, readiness timeout and descendant cleanup. Command tests use explicitly fake drivers; they verify the runner's failure handling, not product behavior.

```sh
python -m unittest discover -s prototypes/overnight/validation/tests -v
python -m py_compile prototypes/overnight/validation/*.py prototypes/overnight/validation/tests/*.py
bash -n prototypes/overnight/validation/run.sh
```

The standalone unit-test command passed all 18 tests. Python compilation and shell syntax validation exited 0. The run script invokes the same unit-test command relative to its own directory.

The measured demonstration was this exact command from the component directory:

```sh
python3 measure.py --output .scratch/measurement-demo.json --label synthetic-python --runs 2 --warmups 1 --duration 0.3 --package measure.py -- python3 measure.py _demo
```

At `2026-10-20T23:15:22.711631+00:00`, on Linux `7.2.8-9-omarchy`, x86_64, Python 3.14.8, the synthetic runs recorded:

| Run | Startup observation | Sampled peak tree RSS | Sampled peak tree PSS |
| --- | --- | --- | --- |
| 1 | 0.153008383 s | 51,191,808 bytes | 37,753,856 bytes |
| 2 | 0.149331963 s | 51,265,536 bytes | 37,749,760 bytes |

Both are fresh processes after a warmup, with primed but uncontrolled filesystem caches. Startup is observation of the synthetic readiness file after the demo's intentional 0.1-second delay. The workload retains a 16 MiB allocation in the parent and 8 MiB in a child, plus interpreter memory. Selected `measure.py` footprint was 8,928 apparent bytes and 12,288 allocated bytes. This excludes Python and system dependencies. These values only demonstrate that the tool records data; concurrent overnight activity prevents any comparative performance conclusion. Sampled peaks may miss shorter peaks.

The final source review removed an unused import and extended malformed adapter-output handling after that demonstration. Measurement source and its reported workload were unchanged. The full 18-test suite was rerun before commit.

## Pending integration and evidence limits

At the availability check, all three sibling branches still had HEAD `69e2583d6fe6063b36f03e9349da9a5096a30ed7`, the planning baseline. No committed core, Qt or Tauri implementation was available to snapshot. No sibling mutable files or builds were read or executed. The permitted fallback is therefore complete runnable validation tools with native comparison pending.

Integration must run `validate.py run-core` against the committed core, reconcile documented node-path/note-schema adapter differences without weakening source assertions, and run the actual restore matcher/store against the edition fixtures. The stateless export checks do not establish durable local identity, unrelated-book isolation, candidate confidence or restore confirmation behavior.

Native Qt/Tauri UI drivers and actual captured traces are pending. `frontend.py` is a driver protocol and observation checker, not a native application or replacement renderer. Its unit-test traces are deliberately fabricated checker inputs and are never presented as UI evidence. The checker cannot authenticate screenshots or a driver's native provenance. Inspect the real driver and captures before calling native behavior verified. Note return, failed-open retention, chapter feedback and book-switch persistence have a documented manual procedure but no native automation result here.

The synthetic image case covers explicit ornaments and repeated substantive images. General mirrored/repeated-image classification, broader CSS, arbitrary malformed recovery and EPUB compatibility remain unverified. The unsupported-table placeholder is a fixture policy, not evidence of full table rendering. Source-path mapping is explicit because CONTRACT.md specifies scalar offsets but not node identifier spelling.

No Qt-versus-Tauri startup, memory or package-size comparison was performed. Use sequential matching release builds after overnight compilation stops, and record installed dependencies separately. No Windows, macOS or mobile platform behavior was tested. No private EPUB, dictionary data, secrets, external service calls, public listeners or existing prototype storage were used.

All source, tests and documentation are confined to the assigned component. The independent worker gates passed. Comparison is intentionally pending under the task's frontend-unavailable allowance; the completion marker uses `complete` for the validation deliverable.
