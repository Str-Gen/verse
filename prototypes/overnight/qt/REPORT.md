# Qt reader prototype report

The native QML reader uses Rust through CXX-Qt 0.10.0. No C++ reader fallback was needed. The prototype accepts the shared version-1 JSON contract, provides conventional pages and selection-driven playback, and remembers stopped source locations and speed per book. Launch the synthetic demo from the repository root with:

```sh
./prototypes/overnight/qt/run.sh --demo
```

Use `--book-json /absolute/path/book.json` for an exported book. The app reopens its last successful input when no book argument is supplied. No EPUB parser was added. The private EPUB and dictionary data were not read, copied, committed, or used in screenshots. Evidence here is fixture-only: two documents, 26 blocks, 720 pieces and two logical chapters with original synthetic text and an SVG.

The tested host is Linux x86_64, Qt 6.12.0, Rust 1.99.0 and GCC 16.2.1. The project is a separate Cargo workspace with a lockfile, pinned Rust toolchain and a two-job build limit. CMake was absent; the [CXX-Qt Cargo build route](https://kdab.github.io/cxx-qt/book/getting-started/4-cargo-executable.html) worked with the installed Qt tools. No system packages or desktop settings were changed. A header-export path mismatch during the include-root cleanup caused one optimized build to fail with `overnight-qt-reader/launch.h: No such file or directory`; matching the native include root and import fixed it. The host's inherited `wayland;xcb` setting also overrode the first smoke-script default. Final tests explicitly select `READER_TEST_PLATFORM` and assert Qt's reported platform; offscreen and Wayland both passed.

The Rust model validates JSON references and maps decoded Unicode scalar source spans to Qt UTF-16 positions. Multiple formatted runs can map to one playback piece. Conventional pages use positions from the real QTextDocument layout; font and window changes retain the current source anchor. Selection, playback return, saved resume and changed-edition confirmation use source spans rather than rendered page numbers. A unique exact context match is offered for confirmation; otherwise an identified chapter start is offered. With neither, the old state stays intact until the reader explicitly confirms a selected replacement location. Context is local, at most seven pieces, and state files are owner-only on Unix.

The theme adapter was adapted from the original Omawrite `src/backend.cpp` and `src/systemtheme.cpp`, with the original MIT license retained. It watches Omarchy's `~/.local/state/omarchy/current/theme/colors.toml`, including ancestor directories and replacement rearming. `READER_THEME_FILE` supplies a safe live test override. Desktop portal color scheme and text scale are read with 150 ms call limits; portal changes and Qt style-hint changes update the fallback. Initial missing-directory watching failed in the first harness run and was fixed by ascending paths lexically before rearming. The final harness passes. Portal signal receiver behavior was tested directly; actual desktop portal signal transport was not simulated or changed.

Verification commands below were run from `prototypes/overnight/qt`. The final verification script is `./tests/check.sh`; it runs the first eight rows sequentially. Runtime logs are ignored under `evidence/runtime/`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` | Pass, no suppressed lint warnings |
| `cargo test --workspace --locked -j 2` | 15 tests pass |
| `cargo test --workspace --doc --locked -j 2` | Pass, zero doctests |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked -j 2` | Pass |
| `./tests/theme.sh` | Pass, actual Qt file-watcher and portal-receiver checks |
| `./tests/whitespace.sh` | Six native QTextDocument checks pass |
| `./tests/smoke.sh` | Pass on Qt offscreen software rendering |
| `READER_TEST_PLATFORM=wayland ./tests/smoke.sh` | Pass on this desktop session |
| `cargo build --release --locked -j 2` | Pass, optimized local build |
| `READER_BINARY=target/release/overnight-qt-reader ./tests/smoke.sh` | Pass on offscreen and Wayland; reported platform asserted |
| `python tests/measure.py > evidence/measurements.json` | Pass, three complete samples and first-frame markers |

The Qt smoke flow compares the actual rendered plain text with Rust's UTF-16 map; inspects heading, italic, bold and image formats; checks list/note text; verifies page navigation and that every character line fits one page; selects a non-BMP character into playback; returns to the last displayed piece; resizes and changes font; and rejects an invalid open without replacing the current book. It also checks actual timer-driven visible playback and return after the mode has rendered. Separate processes verify stopped resume and per-book speed. A synthetic edited revision changes every source node address: recovery stays pending, an attempted save leaves the old resume JSON byte-for-byte unchanged, explicit confirmation adopts the passage, and the next process resumes it stopped.

Rust tests additionally cover words split across inline formatting, missing and ambiguous mappings, image pauses and image resume, per-book isolation, repeated-context chapter fallback, missing/legacy recovery context, explicit replacement confirmation, local image URI handling and rejection of detached notes. Native whitespace probes established Qt's handling of tabs, repeated spaces, empty blocks, carriage returns, nonbreaking spaces and Unicode separators. Their first standalone compile needed `-fPIC` on this host; the retained script includes it.

[Synthetic conventional view, offscreen](evidence/reader-offscreen.png), [synthetic conventional view, Wayland](evidence/reader-wayland.png), [synthetic playback view](evidence/reader-playback.png), and [image/list content](evidence/reader-content.png) are app-window captures, not desktop captures. The source contains no private book text. Tests use fresh isolated state directories and test theme files.

Executable and dependency composition:

The optimized ELF executable is 2,900,024 bytes (2.77 MiB), dynamically linked and not stripped. QML is embedded; the demo JSON/SVG remain external project assets. Direct Qt links include Core, Gui, Qml, Quick, QuickControls2 and DBus; transitive/runtime dependencies include Qt Network/OpenGL, QML plugins and system libraries. The measurement run mapped 152 distinct shared-library files totaling 323,586,091 bytes (308.60 MiB). These are existing host files, not an additional download or deployment bundle. The inherited platform theme was `gtk3`; GTK and NVIDIA driver libraries were mapped even with the software scene graph. The two largest mapped files were NVIDIA's GPU compiler and GL core libraries. See [composition.txt](evidence/composition.txt) for `ldd` and the direct Cargo dependency tree. There are 282 lines across the handwritten production C++/header files, including the verification helpers, excluding native test programs.

Rust owns JSON validation, HTML normalization, source mapping, playback transitions, recovery and persistence. `qml/Main.qml` owns controls, layout-derived page boundaries and the timer. Handwritten C++ is confined to `native/launch.cpp` for the Qt application/engine, `native/theme.cpp` for platform settings, and `native/support.cpp` for document inspection, screenshot and test-exit helpers, plus their headers. There is no handwritten C++ book model or EPUB logic. CXX and CXX-Qt generate the native call wrapper and Rust-backed QObject, properties and invokables. Qt generates moc code, QML registration and embedded QML resource code. Generated sources and all compiler output stay under ignored `target/`. The application links installed Qt libraries and loads QML/platform plugins; this directory is not a portable deployment bundle.

| Trial | Native first frame (ms) | Process count | RSS (MiB) | PSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| 1 | 83 | 1 | 127.34 | 57.55 |
| 2 | 82 | 1 | 127.23 | 57.60 |
| 3 | 82 | 1 | 126.98 | 57.32 |

Measurements use the optimized executable, a synthetic book, the offscreen software backend, a fresh state directory and three sequential trials. RSS/PSS are sampled at about one second after process creation. PSS apportions shared mappings; summing RSS can double-count shared pages. The native first-frame marker measures entry before QGuiApplication through the first `frameSwapped`; it excludes process creation and loader work and does not claim full interaction readiness. Loaded-library file sizes are separate from memory and executable size. Concurrent overnight workloads and cache state were uncontrolled. These figures do not establish a Qt-versus-Tauri advantage. Full trial data and mapped library paths are in [measurements.json](evidence/measurements.json).

Remaining limits and assumptions:

- This proves the Qt/Rust binding and a bounded reader workflow, not every accepted product feature. Chapter feedback/history, a chapter-list control, preparation pulses, ramping and punctuation/paragraph pacing are not implemented. Playback uses a fixed WPM interval and pauses at logical chapter starts, images and unsupported pieces.
- Notes are rendered as inline note blocks and skipped in playback. Popup note links and optional note playback with automatic return are absent. Nonempty detached `notes` arrays are rejected rather than silently discarded.
- Images use a fixed 320 × 120 box plus alt text. Nested list structure becomes simple bullets. Full-document layout is held in memory. Very large books, oversized media, complex tables, publisher CSS, fixed layout, CJK/RTL typography and broad EPUB compatibility were not tested. The producer must resolve visibility and decorative-image classification consistently before export.
- Font and resize checks cover the supplied synthetic desktop geometry. Extreme text scaling, tiny viewports, keyboard-only dialogs and screen-reader behavior need further work. Controls have accessible names, but heading/list/note semantics and reading-order announcements have not been assessed with assistive technology.
- The application has no network asset loading path and escapes supplied text, but publication-local file access is not sandboxed. JSON references to local images are allowed. This is a local prototype, not a hardened document viewer.
- State writes use atomic replacement but no multi-instance locking. Concurrent processes sharing one state directory can overwrite updates. Positions are saved on selection, pause, speed changes, playback stops and normal window close; abrupt termination can lose the latest playback advances. Use separate `--state-dir` values for independent sessions.
- Recovery uses unique exact short-context matching and stable chapter identity/title heuristics. It does not perform semantic or fuzzy matching. Confirmation is required; old states without recovery context remain protected.
- The shared core worker's exporter and private EPUB were not integrated in this independent session. The adapter assumes contract-conforming normalized JSON and remains replaceable by the shared Rust core. Windows, macOS, mobile deployment, distribution packaging, dependency security auditing and installed-size comparisons were not tested.

All task files are under the owned directory. The completion marker will bind this report to the clean committed branch HEAD after final verification and commit.
