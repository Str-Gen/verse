# Independent reader validation

This component uses Python 3.12+ and the standard library. It contains original synthetic EPUB fixtures, an export checker, a native UI adapter protocol, source-offset examples and a Linux measurement tool. It does not import private EPUBs or read another prototype's storage. No dependency installation or compilation is needed.

From the repository root, run the complete local demonstration:

```sh
./prototypes/overnight/validation/run.sh
```

The command runs checker tests, creates 11 synthetic EPUBs, creates an 80-paragraph schema-v1 book and UI action plan, and measures a synthetic Python parent/child process. Outputs stay in this component's ignored `.scratch/`. It launches no browser or public listener. Passing this command verifies the validation tools, not the core or either frontend.

All further commands assume this directory is current. The generated book at `.scratch/demo.json` is ready for either frontend's documented `--book-json` entry point. Its content is original and redistributable under the repository license.

## Core export checks

```sh
python3 validate.py fixtures --output .scratch/fixtures
python3 validate.py run-core --output .scratch/core-results -- /absolute/path/to/reader-core
python3 validate.py validate --case inline_unicode_hidden --book-json .scratch/core-results/exports/inline_unicode_hidden.json
```

`run-core` invokes `export --epub PATH --output PATH` for every fixture, continues after failures, writes `results.json`, and exits nonzero if any case fails. It also repeats the original edition export to check revision determinism. The executable must already be built locally; the tool never invokes another worker's build. Executable prefixes such as `python3 adapter.py` work after `--`.

The assertions were written from synthetic source and the accepted contract before inspecting a core implementation. `fixtures.py` is the expectation source. Expected words and source spans are explicit for the three structural fixtures. The five edition expectations are independent literal sentences. Expected chapter ranges use lexical word ordinals only inside the oracle. Actual positions are resolved through piece IDs and source spans. Display pieces with the same `word_id` are joined, so intentional splitting can pass while inline-markup word fragmentation fails.

| Fixture | Required behavior |
| --- | --- |
| `inline_unicode_hidden` | Join `un` + emphasized `believ` + `able`; retain all three spans; preserve combining marks and astral scalars; exclude hidden attribute, inline CSS, linked CSS, hidden ancestors and scripts from both reading modes; distinguish two fragment chapters in one file; expose a manual note event and note text. |
| `logical_chapters` | Keep title text outside logical chapters; begin River in `a.xhtml`, continue into `b.xhtml`, then begin Ridge within that file. |
| `images_unsupported` | Skip two explicitly presentational ornaments during playback; pause twice on repeated substantive hill images; show and pause at an unsupported table. Repetition alone must not erase real images. |
| `malformed_xml` | Reject with a diagnostic or return an explicit unsupported placeholder. |
| `missing_resource` | Reject with a diagnostic or return an explicit unsupported placeholder. |
| `not_a_zip` | Reject with a diagnostic and no JSON output. |
| `edition_original` | Original Harbor chapter and unique lantern passage. |
| `edition_unique` | Same title and publisher identifier, inserted text before the unique passage. |
| `edition_ambiguous` | Same passage appears twice; do not silently choose one. |
| `edition_chapter_only` | Passage gone; identified Harbor chapter remains. |
| `edition_no_match` | Neither passage nor chapter remains. |

The image fixture intentionally uses explicit presentation roles and substantive alt text. It does not establish a general repeated/mirrored-image classifier. Unsupported-table output is the selected allowed degradation for this prototype suite, not a ban on a future table renderer. The malformed case only checks importer failure/placeholder behavior. Keeping an already open book requires a separate UI check.

### Source-path adapters

Contract offsets count decoded Unicode scalars. `source_nodes` independently parses the synthetic XML with ElementTree and addresses element children from root `0`, then `/text` or `/tail`. For example, `0/1/1/0/tail` addresses text after the first inline child of the second body element. Event anchors use the element path with a zero-length range. The contract does not mandate this node-path spelling.

If the core chooses a different node identifier, provide an audited JSON object mapping `DOCUMENT#EXPORTED_NODE` to the canonical fixture node path:

```sh
python3 validate.py validate --case inline_unicode_hidden --book-json export.json --node-map node-map.json
```

A map changes only node spelling. It cannot change document, scalar offset, length, expected text or chapter boundaries. For batch execution, use an integration-owned command adapter that preserves the exporter CLI and canonicalizes node identifiers before writing JSON. Record its source and commit. Do not construct mappings by searching for the desired exported word; repeated text makes that circular and ambiguous.

Notes have only an array specified in CONTRACT.md. This checker accepts note `text` or note `blocks[].runs[].text`, and expects note events to carry `note_id`. If the implementation uses another nested representation, an adapter may expose that semantic text/link. Marker text is represented by the note event, not by a lexical word. All non-word events must address their actual XML source element. Empty or unaudited mappings must fail, not turn source validation off.

### Changed-edition policy

```sh
python3 validate.py run-restore --output .scratch/restore-actual.json -- /absolute/path/to/restore-policy-adapter
python3 validate.py restore --results .scratch/restore-actual.json
```

The adapter reads a JSON array from stdin and writes a JSON array to stdout. Input cases contain `id`, revision identities, exact-source existence, unique confident passage candidates and identified chapter candidates. Expected answers are withheld from the subprocess. Each output contains `id`, `action`, `target`, `requires_confirmation`, and `preserve_saved`. See `RESTORE` in `fixtures.py` for six explicit examples.

These policy vectors test what to do with established candidates. They do not test whether a matcher correctly discovers candidates or establishes confidence. For end-to-end integration, save the original edition's `amber` source at `edition.xhtml`, `0/1/1/text`, scalar offset 4, length 5, with quote `The amber lantern rests.` and chapter identity Harbor. Open each edited fixture through the actual application's replacement-edition path. Expect a unique passage offer, chapter fallback for repeated or absent passages, and preserved old location when neither passage nor chapter exists. Confirming recovery may update the saved position; merely offering recovery must not. A second chapter with the same title must not be assumed to identify Harbor.

The fixture books deliberately share a publisher identifier. Test opening unrelated fixture books with distinct saved speeds and positions in the real store. No identity assertion is inferred from one stateless CLI export. A content hash used as local identity may isolate unrelated books but also needs an explicit changed-edition association flow.

## Frontend pagination and passage return

```sh
python3 demo.py --output .scratch/demo.json
python3 frontend.py plan --book-json .scratch/demo.json --output .scratch/ui-plan.json
python3 frontend.py run --book-json .scratch/demo.json --output .scratch/qt-trace.json -- /absolute/path/to/qt-native-ui-driver
python3 frontend.py check --book-json .scratch/demo.json --trace .scratch/qt-trace.json
```

Repeat with an actual Tauri driver. A driver reads the plan from stdin, controls a real foreground native application with isolated storage, and writes one trace JSON object to stdout. Diagnostics go to stderr. The runner starts a separate process group and terminates that group after success, failure or timeout. Drivers must keep the app in that group; detached processes require their own explicit cleanup and a residual-process check. Its action implementation must use user-visible controls or native application test hooks that exercise the same layout/state transitions. Directly assigning fabricated results or implementing a second pagination engine is not valid evidence. A browser rendering the Tauri page is useful supplemental evidence but cannot pass as `tauri-native` evidence.

The plan exercises a fresh stopped launch at 350 WPM, Right/Left page turns, selection of `Passage012`, playback start at that source, three subsequent displayed words, stop and return, reflow at 640 x 480, saved 425 WPM, and stopped restart. The driver must navigate to and select the requested passage through the frontend. Advance playback through the actual scheduler or a documented controllable clock; do not rely on a blind sleep landing on exactly three words.

Trace fields are:

- Top level: `version: 1`, `fixture_revision`, `provenance`, `observations`, `unicode_results`.
- Provenance: `runtime` is `qt-native` or `tauri-native`; include `commit`, executable SHA-256, driver identity, platform, isolated storage path, and capture directory.
- Observations: one per action, in order, with matching `id`, `mode`, boolean `playing`, `wpm`, viewport `[width,height]`, ordered `visible_piece_ids`, `selected_sources`, `displayed_sources`, `errors: []`, and a `capture` filename. Fields irrelevant to an action can be absent; the checker states required fields on failure.
- Unicode results: for each requested vector, report `text`, `utf16_boundaries`, `scalar_boundaries`, and `rejected_utf16`. Execute the frontend's actual offset bridge in both directions. Scalar boundaries inside combining sequences are valid; UTF-16 boundaries inside surrogate pairs must be rejected.

`tests/test_frontend.py` contains a deliberately fabricated checker unit-test trace to illustrate the shape. It is not frontend evidence. The checker validates observations, but cannot authenticate the driver or screenshots. Inspect the driver and captures as part of integration, and retain only synthetic content. This distinction appears in every passing check result.

If native automation is unavailable, use the generated action plan manually and capture each step. Report manual evidence and pending automation separately. Also check opening a malformed book preserves the existing book, note playback returns after its marker, chapter feedback pauses at logical boundaries, and book switching preserves separate speeds/positions. Those extra interactions have no automated native coverage in this component.

## Committed snapshots and comparison

```sh
python3 snapshot.py --worktree /absolute/path/to/core-worktree --component core
```

This reads `git archive HEAD` for only the named component and extracts it under this component's ignored `.scratch/snapshots/COMPONENT/COMMIT/`. It never reads mutable sibling source or build output. The printed path is the local snapshot to inspect and build. A missing committed component fails explicitly; an existing snapshot is not overwritten. Read its local instructions before building. Set `CARGO_BUILD_JOBS=2` and a `CARGO_TARGET_DIR` inside this component's `.scratch/` when building Rust snapshots. Never share a sibling target directory or merge a sibling branch.

After integration, run the core suite first, then feed those same successful exports to both frontends. Record an importer failure separately from a frontend failure. Use `.scratch/demo.json` for independent UI layout checks if importer cases still fail, and label that evidence fixture-only.

[MEASUREMENTS.md](MEASUREMENTS.md) gives the sequential startup, process-tree memory and package-footprint procedure. Compile both variants before measuring. Compare matched release builds, synthetic input, storage state, scale and window dimensions. Record fresh-process/uncontrolled-cache and cache-primed runs separately. The overnight synthetic demo demonstrates the measurement tool only; it does not compare Qt and Tauri.
