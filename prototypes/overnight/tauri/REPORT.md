# Tauri overnight report

The prototype builds and runs in native Linux Tauri/WebKitGTK. It consumes the shared version-1 JSON contract through a small Rust adapter; there is no frontend EPUB parser. All new files are confined to `prototypes/overnight/tauri`. No private EPUB or dictionary content was used in fixtures, screenshots or reports. No push, installation requiring privilege, or production release was performed.

## Delivered behavior

Conventional CSS-column pagination supports page buttons and arrow keys. Source-labelled text fragments map mouse selection into playback, including a word split across emphasis runs. Pause and Escape return to the last displayed source piece. Font/window reflow retains the source position. Native restart restores each book's source anchor and speed, stopped, with a 350 WPM default for new books. The last imported JSON book is stored separately from per-book settings.

The frontend preserves normalized headings, emphasis, lists, local raster images and notes. Real/uncertain images and unsupported content pause; explicit ornaments skip. Manual notes are available, and enabled core note references play their note pieces before returning after the marker. Logical chapter boundaries pause, offer optional feedback and explicit 25 WPM suggestions, and keep per-book history with pace changes. Changed revisions require confirmation of a unique exact context match or identified chapter start. An unresolved or declined recovery preserves the previous saved anchor.

The Rust shell bounds and structurally validates JSON input, provides four narrow IPC commands, and writes owner-only local JSON on Unix. WebKit data and book state live in the component's ignored scratch directory. Publisher HTML is never inserted, publisher scripts never execute, and remote book resources are rejected. Embedded PNG/JPEG/WebP resources are supported; unknown local paths have placeholders.

## Build and launch

From the worktree root:

```sh
./prototypes/overnight/tauri/run.sh
./prototypes/overnight/tauri/run.sh --book-json /absolute/path/book.json
./prototypes/overnight/tauri/build.sh --release
```

`run.sh` builds with two jobs and launches the debug artifact. It defaults `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux. Without that workaround, the first Wayland launch failed before UI testing with GDK protocol error 71. X11 subsequently passed; Wayland with the workaround passed without stderr. This is a tested local driver workaround, not a claim that every WebKitGTK installation needs it.

The browser comparison runs with `./prototypes/overnight/tauri/browser.sh` at `http://127.0.0.1:18772`. Native operation uses embedded local assets and needs no HTTP server. See README.md for core export commands, storage and controls; PORTS.md records official-source code/build changes for Windows, macOS, Android and iOS, all untested.

## Verification commands and evidence

Commands executed from the worktree unless otherwise stated:

```sh
node --test prototypes/overnight/tauri/tests/model.test.mjs
./prototypes/overnight/tauri/build.sh
./prototypes/overnight/tauri/build.sh --release
cargo fmt --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=2 cargo clippy --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --workspace --locked
```

Five JavaScript model tests pass. They cover scalar text reconstruction, joined inline formatting, source recovery and ambiguity, malformed input, and distinct source anchors for split pieces. Three Rust tests pass. They cover the structural contract and rejection of malformed identities, spans and references. Formatting and strict Clippy pass. There is no Rust library/doctest target in this adapter.

The native suite operates inside the actual Tauri WebKitGTK window, including layout, DOM selection, font/narrow reflow, source return, per-book state, feedback, image interruptions, recovery, inert publisher markup and core export integration. It imports the core fixture through Rust IPC. It uses DOM events and application methods; it does not claim external OS-level mouse automation.

The final release binary passed all 41 native checks twice using the same scratch directory, exit 0 and no stderr. The second run compares startup against a clone of the previously persisted anchor captured before any new save. Exact command, run twice:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  READER_SCRATCH="$PWD/prototypes/overnight/tauri/.local/release-final-anchor" \
  ./prototypes/overnight/tauri/src-tauri/target/release/quiet-reader-tauri --self-test
```

The final `./prototypes/overnight/tauri/verify.sh` also exited 0, passing formatting, strict Clippy, 3 Rust tests, 5 model tests and two debug-native runs of 41 checks. Its output is `.evidence/verify-final.log`. Release results are `.local/release-final-anchor/self-test/final-first-result.json` and `self-test.json` in the same directory.

One earlier second-run assertion failed because the test assumed note following would reset to false. The app correctly retained the enabled per-book preference. The test now explicitly disables following before checking skip behavior; both repeated runs pass. This was a test-isolation correction, not a suppressed application failure.

A further normal release launch with no `--book-json` and no self-test flag reopened the last imported core fixture, stopped, with its saved marker selected. I inspected `.evidence/native-last-book-resume.png`; the title and selected unresolved-note marker match the stored core fixture. Hyprland reported `xwayland: false`, confirming a native Wayland window. Environment: Rust 1.99.0, Tauri 2.12.4, Wry 0.57.0, GTK 3.24.52 and WebKitGTK 2.54.1.

Chromium 154.0.8037.97 with Playwright 1.64.1 independently passed six external-input checks and all 41 DOM checks. External input covered actual mouse drag selection, page keys, Space/pause/Escape, font reflow, reload with saved speed/source, and malformed file input retaining the book. No uncaught page errors occurred. Exact invocation:

```sh
PLAYWRIGHT_MODULE=/home/strgenix/.local/share/mise/installs/npm-playwright/1.64.1/node_modules/.mise/playwright@1.64.1/node_modules/playwright/index.mjs \
  node prototypes/overnight/tauri/tests/browser-check.mjs
```

BROWSER-NOTES.md contains browser details. Local output is `.evidence/browser-final.json`. Native and Chromium screenshots in `.evidence/` contain only synthetic content and are ignored by Git. I visually inspected both. The native screenshot shows conventional pagination, normal/emphasized/bold text and the selected source piece without clipped controls.

`verify.sh` combines Rust/model checks and two native launches against the same isolated scratch directory, including persisted stopped restart. It requires an active graphical session.

## Actual core export

The live sibling core CLI was used read-only:

```sh
/home/strgenix/speedreading/overnight-20261021/worktrees/core/prototypes/overnight/core/target/debug/reader-core \
  demo --output prototypes/overnight/tauri/.local/core/demo.json \
  --store prototypes/overnight/tauri/.local/core/store
```

It emitted version 1 with 2 documents, 48 main pieces, 3 logical chapters, 1 note and 4 raster resources. The exported synthetic snapshot is `web/core-demo.json`, SHA-256 `e85a6fce1c839e9f615102d5fb02566c2bbd6de205e7d88339e656b1f9878f8d`. Native checks verify multi-run mapping, five rendered image occurrences, manual notes, disabled note following, enabled note playback with return, and an unresolved note pause. This proves an actual core export crosses the adapter; it is not proof against the private EPUB or a broad corpus. The core was under concurrent development, so integration should regenerate this fixture and rerun checks against its final commit.

The frontend remains standalone. Direct core-library wiring and shared playback/recovery policy consolidation are documented in README.md. No sibling source or manifest was edited.

## Size and runtime observations

Measured executable sizes are **7,021,744 bytes** for release and **22,232,896 bytes** for the final debug build. `ldd` resolved 143 distinct shared-library files totaling **271,979,120 bytes**, with no missing library. These totals exclude WebKit helper executables and other package assets. No installer bundle was generated. Shared-library file totals are distinct from the executable and do not represent a download size, incremental installation size, or complete runtime-package footprint. The ignored target directory occupied about 2.5 GiB after both builds and checks. Build intermediates are not shipped assets.

The measurement script starts a fresh isolated process with the same synthetic book, verifies the saved source/speed, waits five seconds, and sums `/proc` RSS/PSS over its descendants. Readiness means the first persisted state after initial frontend layout, not first paint. The final release sample reported **353.83 ms** to that readiness point, **617,020 KiB aggregate RSS** and **327,598 KiB aggregate PSS** across the reader, WebKitNetworkProcess and WebKitWebProcess. RSS double-counts shared pages; PSS apportions them. The synthetic state was verified, the process remained alive at sampling, and stderr was empty. The compositor reported a 1517 × 832 native Wayland window. The machine had 32 logical CPUs and a one-minute load average of about 3.04.

Exact measurement command:

```sh
python prototypes/overnight/tauri/tests/measure-process.py \
  prototypes/overnight/tauri/src-tauri/target/release/quiet-reader-tauri
```

Local output is `.evidence/measurement-release.json`. Only one final release sample was collected during other overnight work. There is no Qt comparison, performance verdict, production tuning claim or stable startup estimate.

## Remaining limits

- Windows, macOS, Android and iOS were not built or run. There is no signed installer, mobile project or production release. Platform storage and mobile lifecycle/import work remain.
- The private EPUB was not opened in this frontend. Whole-book CSS-column DOM layout is unprofiled on long books, and a broad EPUB corpus was not exercised.
- Source validity against original EPUB text, hidden-content decisions, identity and ornament classification belong to the core. The adapter validates shape, not those semantics. The native fixture tests do not establish arbitrary EPUB fidelity.
- The accepted preparation pulse, 30-second speed ramp and vocabulary capture are not implemented here. Playback has fixed-center presentation and punctuation/paragraph pauses.
- Partial-word selections begin at their containing source piece. Nested note references do not recurse. Restart during note playback restores the main-text marker, not an internal note word.
- Recovery currently uses an exact five-piece context and stable chapter ID with confirmation. It must be reconciled with the shared Rust core's final recovery implementation.
- Invalid in-app imports preserve the current book. Corrupted startup scratch JSON fails visibly but is not automatically repaired. Concurrent processes must use different scratch directories. Power-loss durability and close-during-write behavior remain unverified.
- Browser persistence has browser quota limits and is separate from native scratch storage. Native WebKitGTK tests and Chromium input tests are reported separately.

The native dependency gate is satisfied on this machine with the documented renderer workaround. The remaining items are prototype and porting limits, not claims of completed platform support.
