# Integrated overnight prototypes

From the repository root:

```sh
./prototypes/overnight/qt/run.sh --demo
./prototypes/overnight/tauri/run.sh --demo
```

Both commands build the same pinned Rust core, export its synthetic EPUB, build the selected frontend, and launch it. Use `--epub /absolute/path/book.epub` to import a local DRM-free EPUB or `--book-json /absolute/path/export.json` to open an existing export. Input paths are resolved before changing directories. With no arguments the frontend retains its own last-book behavior. Run only one instance per frontend storage directory.

`export.sh` exposes the core CLI unchanged. `launch.sh` owns import and the persistent canonical-path identity store in `.local/identities`. Exports live under `.local` with a filename derived from the EPUB path. Replacing the bytes at that path retains book identity while changing revision. The two renderer projects remain separate Cargo projects; there is one EPUB importer and reusable Rust library in `../core`. This prototype shares content through a versioned JSON process boundary. Frontend persistence, playback and recovery policy still need consolidation into the Rust application layer.

## Verification

```sh
cargo +1.99.0 test --manifest-path prototypes/overnight/core/Cargo.toml --locked -j 2
python prototypes/overnight/integration/check_core.py
python prototypes/overnight/integration/check_recovery.py
./prototypes/overnight/qt/tests/check.sh
./prototypes/overnight/integration/check_native.sh
```

The native script requires the installed Wayland session, Qt 6 and WebKitGTK. It tests real Qt offscreen and Wayland renderers, then runs Tauri's real WebKitGTK suite twice. It generates fresh synthetic snapshots in the Tauri web directory before building. Qt's inherited fixture suite also exercises page layout, timer-driven playback, failed open, per-book persistence and confirmation across restarted processes. Its theme suite uses a synthetic override and tests real Qt file watching, including atomic replacement and initially missing ancestors. No desktop theme is modified.

`check_core.py` runs the independent worker's oracle through `validation_adapter.py`. The adapter audits source spans against original XML, translates node spelling, represents inline note references as the oracle's separate note blocks, and omits explicitly classified ornament events because the oracle expects them absent from playback. It does not alter scalar offsets, word text, chapter boundaries or image classification. The unchanged SVG fixture fails its rendering expectation because the core emits explicit unsupported placeholders. A separate PNG fixture passes the same image/ornament expectations. The wrapper succeeds only with that exact known failure and a passing raster case. Raw results retain the failure.

`check_recovery.py` calls the actual CLI/store against independent editions and checks that proposals never modify saved state. The core's eight-word confidence threshold sends the short unique fixture to chapter confirmation. Qt and Tauri currently use shorter exact contexts. This is an open policy difference, not a shared-recovery completion claim.

The independent worker's generic ten-action trace protocol has no completed Qt/Tauri driver. Its independent content fixtures do run through both native renderers with source-selection and return assertions. Browser tests, mobile ports, broad publication compatibility and user visual acceptance are not implied by native tests.

For sequential resource samples, build both release artifacts, wait until no builds remain, then run `python prototypes/overnight/integration/measure.py`. It refuses to start while build processes exist. Outputs and synthetic screenshots are in ignored `evidence/`; the final control-directory archive preserves them. See the root `MORNING-REPORT.md` for observed outcomes and unresolved gates.
