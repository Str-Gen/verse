# Tauri reader prototype

A Linux Tauri 2 frontend over version-1 normalized book JSON. The Rust adapter loads JSON and persists the last book and source-based reading state. The frontend never parses EPUB or inserts publisher HTML. All bundled text and artwork are synthetic.

## Run

From the repository root:

```sh
./prototypes/overnight/tauri/run.sh
```

This builds the standalone Cargo workspace with two compilation jobs, then opens the synthetic book on first launch. On Linux the script defaults `WEBKIT_DISABLE_DMABUF_RENDERER=1` because the default DMA-BUF path failed on this machine; an explicit environment value overrides it. Rust 1.99, GTK 3 and WebKitGTK 4.1 development packages are required. `src-tauri/Cargo.lock` pins dependencies. The component does not use a frontend package manager or runtime framework.

Open a core export from the command line:

```sh
./prototypes/overnight/tauri/run.sh --book-json /absolute/path/book.json
```

The in-app **Open book JSON** action accepts the same format, up to 32 MiB. It validates the file before replacing the current book. The Rust command-line limit is 64 MiB. Normal startup restores the last loaded book, its source position and speed, with playback stopped. A fresh book starts at 350 WPM.

For a browser-only comparison:

```sh
./prototypes/overnight/tauri/browser.sh
```

Open `http://127.0.0.1:18772`. This serves only the bundled web directory and binds only to loopback. Browser persistence uses its own `overnight-tauri-browser-v1` localStorage key; it does not use native storage. Browsers impose storage quotas, so use the native adapter for large books. The browser server does not read arbitrary local files.

## Reading

- Left and Right turn conventional pages. Select or click a passage, then press Space to play from its source piece. Space pauses; Escape returns to the last displayed passage.
- The quiet footer’s **⋯** button exposes file and reading controls. The default reading font is IBM Plex Mono with JetBrains Mono Nerd Font and monospace fallbacks, at 20 pixels in a roughly 65-character column. Previously saved font sizes remain respected.
- Images retain their proportions, shrink to fit the reading area, and are never enlarged. Alt text remains accessible without a duplicate caption; unavailable resources retain a visible fallback.
- Reading settings expose chapter navigation, font size, per-book speed, optional note following and chapter feedback. Font or window changes reflow around the selected source rather than retaining a page number.
- Image and unsupported-content items pause. Space continues; Escape shows their conventional context. Explicit ornaments skip playback. Image classification belongs to the core.
- Note links open on demand. With note following enabled, the core's `note_id` references play `notes[].pieces`, then continue after the marker. Nested notes are not followed recursively. This prototype also handles inline `kind: note` blocks in the small local fixture.
- Logical chapter boundaries pause, including the end of the last chapter. Save and continue records the two subjective answers; Skip and continue stores no answers. Disabling questions retains the pause. Accepting the displayed suggestion changes speed by 25 WPM. History is local and can be cleared for the active book.
- A changed revision never silently reuses an old array index. An exactly matching, unique five-piece context is offered for confirmation; otherwise an exact chapter-ID match is offered. With neither match, the old saved anchor remains until the reader explicitly chooses a new passage. This is a conservative prototype matcher, not the final shared-core recovery implementation.

Source addresses use document, node, scalar offset and length plus revision. JavaScript splits strings by Unicode scalar value for source mapping; DOM selections map through source-labelled text fragments. A word spanning emphasis runs remains one playback piece. Pixel positions and page numbers are only transient layout data.

## Core integration

After the components are merged, run the core CLI separately:

```sh
./prototypes/overnight/core/target/debug/reader-core export \
  --epub /absolute/path/book.epub \
  --output /absolute/path/tauri/.local/import/book.json \
  --store /absolute/path/tauri/.local/import/core-store
./prototypes/overnight/tauri/run.sh --book-json /absolute/path/tauri/.local/import/book.json
```

Use an ignored path under this component for private exports. `reader-core demo --output PATH --store PATH` generates a synthetic export instead. The adapter consumes the contract's required fields and the core's additive `resources`, `run.note_id`, `piece.note_id` and `notes[].blocks/pieces` fields. Resources resolve only to bounded embedded PNG/JPEG/WebP data. Arbitrary file paths, remote image URLs and publisher SVG do not load. An unavailable image has an explicit placeholder.

`web/core-demo.json` is a synthetic snapshot exported by the actual sibling core CLI during this session, not a frontend importer result. `web/demo.json` is the longer local fixture generated by `python prototypes/overnight/tauri/make-demo.py`. Keep the core snapshot synthetic when refreshing it.

For direct Rust library integration, add a path dependency on `../core` relative to this component's root, or `../../core` from `src-tauri/Cargo.toml`. Replace JSON-only opening with a narrow command calling `reader_core::import_epub`, and serialize its `Book` into this same boundary. Reuse the core's identity, recovery and persistence functions after reconciling their interfaces. The current build deliberately remains standalone and does not link a mutable sibling worktree. Frontend playback timing and recovery are currently adapter code, not claimed shared-core implementations.

## Verification

`./prototypes/overnight/tauri/verify.sh` runs the Rust gates, model tests and two real native self-test launches against one isolated scratch directory. It requires an active graphical session. The second launch checks stopped resume from the persisted source anchor.

```sh
node --test prototypes/overnight/tauri/tests/model.test.mjs
./prototypes/overnight/tauri/build.sh
READER_SCRATCH="$PWD/prototypes/overnight/tauri/.local/check-$(date +%s)" \
  ./prototypes/overnight/tauri/run.sh --self-test
```

The native self-test runs in the real Tauri WebKitGTK window, exercises DOM layout and controls, imports the core's synthetic fixture through Rust IPC, writes `self-test/self-test.json` under the selected scratch root and exits with success or failure. Use a fresh scratch path for each run. Tests assert the synthetic fixture's content and count, so do not point this mode at a private book. The native shell forces the bundled synthetic starting fixture in self-test mode.

`tests/browser-check.mjs` drives actual Chromium mouse and keyboard events when Playwright is available. See BROWSER-NOTES.md for the exact local invocation. Chromium is supplemental evidence, not a substitute for native WebKitGTK.

## Storage and security

The default scratch directory is this component's ignored `.local`, including the WebKit profile/cache. Override it with `READER_SCRATCH` to isolate a run. Native JSON files are owner-only on Unix; no production library or other prototype storage is used. Saved state includes source spans, nearby passage text, per-book settings and feedback. Book contents remain local. Do not commit scratch files or include private screenshots in reports.

The frontend constructs text nodes and known semantic elements. It has no publisher-script path. CSP and native navigation restrictions reject remote content. There are no shell/filesystem plugins exposed to JavaScript and no external fonts, analytics or paid services. Build output and screenshots are locally ignored.

## Prototype boundaries

This is a runnable comparison artifact, not a release. It lays out the whole book into CSS columns, which can consume substantial DOM memory on long books. It has not been validated against a broad EPUB corpus. The normalizer decides visibility, semantic content, chapter identity and ornament classification before rendering.

Playback includes sentence/paragraph pauses and a fixed center guide. It does not yet reproduce the accepted two-second preparation pulse, 30-second speed ramp, compound timing policy or recent-vocabulary capture. Explicit source-piece selection is verified; arbitrary partial-word selections start at their containing piece. Note interruption returns to the main-text marker rather than reopening a note at an internal word on restart.

The JSON adapter validates structure but does not independently verify source addresses against an EPUB. It trusts the core's decoded-source semantics. Changed-edition matching currently uses a five-piece exact context and chapter IDs, with confirmation. Import errors keep the current book. Startup errors are reported, but automatic repair of corrupted scratch JSON is not implemented.

Native persistence writes on position changes; close-during-write durability and power-loss behavior remain unverified. Only one process should use a scratch directory at a time. See REPORT.md for measured results, platform-specific launch constraints and remaining untested gates. See PORTS.md for official-source porting steps; Windows, macOS, Android and iOS are not claimed to work.

## Integrated core launcher

`./prototypes/overnight/tauri/run.sh --demo` now regenerates the shared Rust core's synthetic EPUB export. `--epub PATH` imports a local EPUB through the same core CLI and identity store used by Qt. `--book-json PATH` still opens an existing export, and no arguments retain last-book behavior. See `../integration/README.md` for reproducible integrated checks and known schema boundaries. The native suite also consumes the independent worker's hidden-text/astral-Unicode export. Worker REPORT.md is historical evidence from before integration.
