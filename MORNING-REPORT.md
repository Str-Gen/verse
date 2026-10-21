# Overnight integration, 2026-10-21

Historical report. The legacy independent oracle was subsequently removed. Use [current verification instructions](prototypes/overnight/integration/README.md#verification); commands and outcomes below describe the original overnight run.

All four worker branches are merged into `overnight/20261021-integration` with real merge commits. Qt/QML with Rust and Tauri/WebKitGTK both run on this Linux/Omarchy host and consume the same Rust EPUB content core. This is a runnable prototype integration, not a production release or complete accepted-feature implementation.

## Run it

From `/home/strgenix/speedreading/overnight-20261021/worktrees/integration`:

```sh
./prototypes/overnight/qt/run.sh --demo
./prototypes/overnight/tauri/run.sh --demo
```

For either command, replace `--demo` with `--epub /absolute/path/book.epub` or `--book-json /absolute/path/export.json`. With no arguments each frontend retains its own last-book behavior. The launchers build locally with Rust 1.99.0 and two compiler jobs. Qt uses installed Qt 6.12 and CXX-Qt 0.10; Tauri uses installed WebKitGTK and the tested Linux `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround. No privileged packages were installed.

Both launchers call `prototypes/overnight/integration/export.sh`, which builds and invokes the ONE Rust core CLI/library. Shared canonical-path identities and exports live in the integration component's ignored `.local`. Frontend state remains separate. Qt accepts `--state-dir`; Tauri accepts `READER_SCRATCH`. Do not run concurrent instances against the same state directory. This is a versioned JSON process boundary, not yet shared Rust application policy in both shells.

## Integration changes

- Merged core, Qt, Tauri, then validation. Every worker marker matched its clean branch HEAD. No worker changed files outside its owned component; no merge conflict required resolution.
- Connected both launchers to fresh core exports and fixed relative input-path handling.
- Fixed an independently detected whole-word bug. Unicode word segmentation had split `A😀B` and attached `𝄞` across whitespace to a previous word. Playback now preserves whitespace-delimited passages, including formatted multi-node words, scalar spans and explicitly requested compound splits. A Rust regression and both native renderers verify the astral-symbol case. The core synthetic demo now has 49 main pieces rather than 48.
- Qt now resolves embedded core PNG/JPEG/WebP resources and detached notes. Inline note links open a manual dialog; detached notes stay outside the main playback stream. Source mapping still converts decoded scalar offsets to native UTF-16 layout positions.
- Added an audited independent-checker adapter for XML node spelling, inline-note representation and explicit ornament-event omission. It verifies original text spans against source XML before translation. It does not rewrite expected words, offsets, chapter boundaries or image classifications.
- Updated the planning map and tickets 06, 07 and 11 with evidence. They remain open. Qt/QML with Rust remains the preferred Linux direction, with Tauri as the second frontend. No user acceptance of visual quality is inferred.

## Executed checks

All evidence paths below are relative to this worktree. `prototypes/overnight/integration/evidence/` is ignored by Git and preserved in the control-directory archive described below. Worker REPORT.md files remain historical records of their own sessions.

| Check | Actual outcome | Evidence |
| --- | --- | --- |
| Core `cargo +1.99.0 test --manifest-path prototypes/overnight/core/Cargo.toml --locked -j 2` | 18 module tests + 14 integration tests pass; 0 doctests | `evidence/core-tests.log` under integration component |
| Core formatting, strict Clippy, locked build, warning-free API documentation | Pass | `core-clippy.log`, `core-build.log`, `core-doc.log` |
| Independent validator unit suite | 18 tests pass | `validation-tests.log` |
| `python prototypes/overnight/integration/check_core.py` | Original independent suite: 12/13 pass, one retained SVG-image failure. Separate PNG equivalent passes the same image/ornament/table expectations. Wrapper exits 0 only when that exact known failure remains and the PNG case passes. | `independent/results.json`, `independent/raster-result.json`, `independent.log` |
| `python prototypes/overnight/integration/check_recovery.py` | 5 real CLI/store scenarios pass their recorded conservative expectations; saved files remain byte-identical during every recovery proposal | `recovery.json` |
| `./prototypes/overnight/qt/tests/check.sh` | Formatting, strict Clippy, 15 model tests, doctests, docs, real theme watcher, six native whitespace checks and five-process native smoke/recovery sequence pass | `qt-check-final.log` |
| `./prototypes/overnight/integration/check_native.sh` | Qt core export: 9 native checks each on offscreen and Wayland. Independent Unicode/hidden-text export: 8 each. Tauri: strict Clippy, 3 Rust tests, 5 JS model tests and two real WebKitGTK runs of 45 checks pass | `native-final.log`, `qt-offscreen-*.log`, `qt-wayland-*.log` |
| Qt release Wayland smoke and core export | Pass, including restarted recovery sequence and 8 core checks without screenshot assertion | `qt-release-smoke.log`, `qt-release-core.log` |
| Tauri release `--self-test` | 45 real WebKitGTK checks pass; no stderr | `tauri-release-result.json`, `tauri-release-native.log` |
| Shell syntax, Python compilation, `git diff --check` | Pass after final edits | Checked in integration session |

The native checks cover real layout, source selection into playback and return, per-book state/speed, stopped restart, changed-revision confirmation, image/unsupported pauses and logical chapters. Qt's synthetic theme test checks initially absent directories, atomic file replacement, watcher rearming and fallback colors. Portal receiver behavior is tested directly; actual portal signal transport was not simulated. Tauri has no Omarchy colors-file watcher yet.

The independent worker's generic ten-action frontend trace protocol still has no Qt/Tauri driver. Its content fixtures are tested in the real native application hooks instead. This is not a claim that the generic trace suite passed. The Tauri tests use DOM events/application methods inside actual WebKitGTK, not external OS mouse automation. Chromium/browser tests were not rerun here and are not counted as native evidence. No mobile or other desktop port was tested.

The real core recovery test offers a chapter for the short unique passage as well as ambiguous/absent passages. Its confidence threshold requires eight words and 32 scalars; the independent unique fixture is shorter. A no-match edition preserves the old location. Longer unique-context recovery is covered by core tests. Qt and Tauri still use shorter, different context matchers. This policy consolidation remains open.

## Synthetic captures

I inspected the native Qt and Tauri captures. They contain only generated synthetic content. Inspection is not user visual acceptance.

- `prototypes/overnight/integration/evidence/qt-wayland-core.png`
- `prototypes/overnight/integration/evidence/qt-wayland-independent.png`
- `prototypes/overnight/integration/evidence/qt-release-native.png`
- `prototypes/overnight/integration/evidence/tauri-release-native.png`
- Offscreen equivalents and Qt fixture playback/content captures are in the same directory.

## Resource evidence

`python prototypes/overnight/integration/measure.py` ran sequentially after all workers completed and compilation stopped. It refuses to start a trial when it observes a build process. Three fresh-process/fresh-storage release runs per frontend used the same core JSON hash and native Wayland window size, 1517 × 832 logical pixels. Both were reported as native Wayland, not XWayland. First runs had uncontrolled caches; later runs had primed filesystem caches.

| Sample | Qt | Tauri |
| --- | ---: | ---: |
| Executable bytes | 2,925,904 | 7,022,928 |
| Installed linked-library file bytes | 114,640,336 | 271,979,120 |
| Process-tree RSS after five seconds, MiB | 261.2 / 195.9 / 196.1 | 589.8 / 588.4 / 593.2 |
| Process-tree PSS after five seconds, MiB | 143.6 / 87.1 / 87.2 | 306.0 / 307.6 / 307.6 |
| Spawn to observed mapped window, ms | 292.3 / 267.1 / 269.0 | 218.9 / 220.9 / 221.7 |

These measurements are explicitly marked contaminated by uncontrolled desktop/background activity. No compiler was observed at trial boundaries. One-minute load varied roughly 0.72–0.93. The first Qt sample had higher memory than later samples; no cause is established. RSS double-counts shared pages; PSS apportions them. These are steady samples, not peaks. Mapped-window timing is not first content or first paint and includes polling overhead. Release profiles differ: Tauri uses thin LTO and stripping; Qt uses Cargo's default release settings. These results support only local observations, not a causal performance verdict.

No installer bundle was built. Executable and linked-library totals are separate; they exclude helper executables, Qt plugins and other package assets and are not download or incremental installation sizes. Full per-process counts, binary/book hashes, loaded-window dimensions, load averages, state locations and conditions are in `prototypes/overnight/integration/evidence/measurements.json`.

## Remaining work

1. The original independent SVG case expects rendered images and skipped ornaments; the core deliberately emits five explicit unsupported events, including its table. Supporting safe SVG rendering or agreeing a different capability expectation remains open. The original failing result is retained. Supported raster classification passes, including repeated substantive images and explicit ornaments; broad visual classification remains unproven.
2. Consolidate playback, recovery and persistence policy in Rust. Current confidence thresholds differ, even though all tested proposals require confirmation and preserve old state.
3. Qt still lacks optional note playback with return and chapter feedback/history. Tauri has these prototype behaviors, but lacks the Omarchy live theme-file adapter. Neither completes the accepted preparation pulse, speed ramp, vocabulary capture/definition/practice scope or production packaging.
4. Complete the independent ten-action native trace drivers and broaden EPUB/CSS testing. The normalizer has a bounded CSS subset, no arbitrary SVG/table/media renderer, no broad malicious-corpus test and no tested unspaced-language segmentation policy. Whitespace-delimited passages are not a general multilingual linguistic tokenizer.
5. Qt embedded-image support is PNG/JPEG/WebP, while the core also understands GIF. That capability mismatch remains. Malformed startup state, simultaneous writers, power-loss durability and long-book memory behavior need further work.
6. User typography/visual review is still pending. This session used synthetic books only and did not repeat the core worker's private-book check. Windows, macOS, Android and iOS remain untested.

## Merge and cleanup status

| Worker | Original HEAD | Merge | Cleanup |
| --- | --- | --- | --- |
| core | `0f36d23478a88a9d4bb64aa6bc157c42750b9de6` | Retained by real merge | Retain worktree: independent SVG conformance and recovery policy differences remain open |
| Qt | `f3a00de9c1f3f97c0832e048a632f0ba929dcece` | Retained by real merge | Removed with ordinary `git worktree remove` after verified backup and clean/ancestry recheck |
| Tauri | `69873f1efea9f395f97db64616766d3ec1a819d8` | Retained by real merge | Removed with ordinary `git worktree remove` after verified backup and clean/ancestry recheck |
| validation | `38e0b5f958c54a85b8f0e6515945322558e2b32d` | Retained by real merge | Retain worktree: original suite has one failing case and generic native trace drivers remain pending |

Qt and Tauri's relevant integrated native/build checks passed. Those two worker directories were removed successfully without force. Both one-command launchers were then verified from the integration tree, including Tauri launched from `/tmp`. Core and validation are retained conservatively because their remaining integration gates are not fully green. All branch refs remain. No worker checkout was modified to make it clean.

The primary checkout remains on `planning/epub-reader`. The integration tree and all older/unlisted prototype/research trees remain. No push, publish, external-service message, paid API, parent `.key` access, privileged installation or desktop-theme change occurred.

Backups and evidence are in `/home/strgenix/speedreading/overnight-20261021/archives/`:

- `workers-before-cleanup.bundle`, verified with `git bundle verify`, contains all four worker branch refs plus the integrated-code commit and their history.
- `worker-1-core-evidence.tar.gz`, `worker-2-qt-evidence.tar.gz`, `worker-3-tauri-evidence.tar.gz`, and `worker-4-validation-evidence.tar.gz` preserve each original component, REPORT.md, completion marker, and non-build evidence/state/logs. Only build/dependency/cache directories were excluded. All four archives were read back before removal; hashes and inventories are in `worker-archive-inventory.json`.
- `integration-handoff.tar.gz` preserves the final integrated component sources, synthetic evidence/logs/state, this report and the updated planning map/tickets. Build outputs are excluded.

The control directory also contains `integration-inspection.json`, `cleanup-result.json`, a copy of this report, and `integration-result.json` with the final integration HEAD, checks, removed/retained worktrees and archive hashes. The final report-only commit follows the bundle's integrated-code commit. All worker commits remain ancestors of final integration HEAD and every worker branch ref remains intact.
