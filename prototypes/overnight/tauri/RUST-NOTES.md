# Rust adapter evidence

The native adapter now links `reader-core`. `prepare_session` validates and adapts the book, resolves the saved anchor in Rust, and supplies playback action tables. `save_state` captures the source anchor from the Rust book rather than trusting a JavaScript-generated location. The shared writer syncs file contents and replaces a unique temporary sibling. State is capped at 1 MiB; the book cache remains capped at 64 MiB. Legacy short-context recovery is no longer used by the native app. The standalone browser preview retains its JavaScript compatibility model.

The remaining notes below record the original overnight implementation and measurements.

The standalone workspace pins Rust 1.99.0. Cargo.lock resolved Tauri 2.12.4 and Wry 0.57.0. The adapter consumes normalized version-1 JSON and performs no EPUB parsing. Four commands expose boot, normalized-book import, state save, and synthetic self-test reporting. No generic filesystem command is exposed.

Book input is capped at 64 MiB and state at 8 MiB. Rust checks collection shapes, unique document/block/piece/chapter identities, source-span integers and document references, run scalar lengths, piece block references, and chapter endpoints. Extra fields remain available to the frontend. This is structural validation, not proof against original EPUB text. Chapter ordering, note semantics and resource support remain frontend/core responsibilities.

`READER_SCRATCH` overrides the default owned `.local` directory. Book/state JSON and WebKit storage remain there. On Unix, the active scratch directory has mode 0700 and new JSON files mode 0600. Writes replace files through a temporary sibling while a mutex serializes commands. File and parent-directory syncing supports recovery from interrupted writes. Per-book OS locks exclude simultaneous opens of the same book, and separate records prevent different books from overwriting each other. Self-tests always load the bundled synthetic demo and use a `self-test` subdirectory. Use a fresh scratch root for each test to avoid restoring prior test state.

The webview uses local embedded frontend assets, a restrictive CSP, and navigation restricted to the local Tauri origin. Publisher scripts and remote resource loading are disabled. The executable's default scratch path is derived from the build-time workspace, so packaged releases need a platform application-data path instead.

Commands executed from the worktree:

```sh
./prototypes/overnight/tauri/build.sh
CARGO_BUILD_JOBS=2 cargo clippy --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --workspace --locked
cargo fmt --manifest-path prototypes/overnight/tauri/src-tauri/Cargo.toml --all -- --check
GDK_BACKEND=x11 READER_SCRATCH="$PWD/prototypes/overnight/tauri/.local/native-1" ./prototypes/overnight/tauri/run.sh --self-test
```

Build, strict Clippy and formatting passed. Three Rust tests passed, covering acceptance of the synthetic contract and rejection of unknown schemas, missing identities/pieces, bad source spans, unknown source documents and duplicate piece IDs. There is no library target or doctest suite.

The first actual native X11/WebKit run exited 0 with 31 frontend DOM/state checks passing and no frontend errors. Result lives in ignored `.local/native-1/self-test/self-test.json`. GTK was 3.24.52, WebKitGTK was 2.54.1. Initial default Wayland launch failed before testing with `Gdk Error 71 (Protocol error)`. X11 emitted two GBM allocation warnings despite successful DOM checks. Later final verification belongs in REPORT.md.

An initial debug binary measured 21,943,496 bytes. `ldd` resolved 143 shared-library files totaling 271,979,120 bytes, with no missing dependency. These are shared library file sizes on this machine, not an installer size or incremental download requirement. Later frontend embedding or Rust edits can change binary size. Debug build intermediates measured 1.4 GiB after build/clippy/tests. At that stage release mode had not yet been tested. Other operating systems remain untested.

A second native run passed all 39 checks on Wayland with no stderr:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 READER_SCRATCH="$PWD/prototypes/overnight/tauri/.local/native-wayland-2" ./prototypes/overnight/tauri/run.sh --self-test
```

The added checks cover the shared core's synthetic export, embedded raster resources, manual notes, note playback and return. `run.sh` now defaults `WEBKIT_DISABLE_DMABUF_RENDERER=1` only on Linux, preserving any explicitly supplied value. This driver workaround disables the DMA-BUF renderer. It does not switch away from WebKitGTK or the native Wayland window. The post-change debug executable measured 22,232,640 bytes. A fresh conventional synthetic session was left running for screenshot and process measurements.

A direct launch with a synthetic `{"schema_version":999}` input exited 2 with the generic structural-validation error before creating any persisted book. No input content appeared in the error.

## Final release run

`./prototypes/overnight/tauri/build.sh --release` passed with two compilation jobs. The initial release build took 1m 51s according to Cargo; subsequent frontend re-embedding/linking took about 30s. This timing is a concurrent overnight observation, not a benchmark comparison.

The final release executable is 7,021,744 bytes, stripped with thin LTO. Its 143 resolved shared-library files total 271,979,120 bytes, with none missing. Combined debug/release build intermediates measured 2.4 GiB.

The final release ran the following command twice against the same persisted scratch directory. Both processes exited 0 and each reported 41 passing checks with no frontend errors or stderr:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 READER_SCRATCH="$PWD/prototypes/overnight/tauri/.local/release-final-anchor" ./prototypes/overnight/tauri/src-tauri/target/release/quiet-reader-tauri --self-test
```

Ignored results are `.local/release-final-anchor/self-test/final-first-result.json` and `self-test.json`. The startup assertion compares the restored source anchor against a snapshot taken before reflow or state writes, and requires stopped conventional mode. Thus the second process exercises actual native persistence. A preliminary repeated run exposed a test assumption that notes would be disabled despite a persisted enabled preference. The test now explicitly disables note following for its skip-policy assertion; no application behavior was changed for that failure.

Rust source was unchanged after its final successful three-test suite, strict Clippy and formatting checks. Later changes affected embedded frontend assets and launch behavior, and the final release runs above exercised those changes. No non-Linux port was built or run.
