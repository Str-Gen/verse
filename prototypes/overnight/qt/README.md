# Native Qt reader proof

This standalone Cargo project keeps reading state, source mapping, playback and persistence in Rust. CXX-Qt 0.10 generates the QObject bridge. QML owns the interface and pagination over Qt's native rich-text layout. It consumes normalized version-1 book JSON; it does not parse EPUBs.

From the repository root, launch the synthetic demo with one command:

```sh
./prototypes/overnight/qt/run.sh --demo
```

The launcher exports the shared Rust core synthetic EPUB for `--demo`, then builds with at most two compiler jobs. `--epub PATH` uses the same importer and canonical-path identity store as Tauri. See `../integration/README.md`. Requirements are Rust 1.99.0, a C++ compiler, Qt 6 development tools and the Core, Gui, Qml, Quick, QuickControls2, DBus and Quick Dialogs runtime modules. This machine has Qt 6.12. No CMake or privileged installation is needed. Cargo dependencies are locked. The demo and its SVG are original, redistributable synthetic content.

For an exported book:

```sh
./prototypes/overnight/qt/run.sh --book-json /absolute/path/book.json
```

Relative input paths are relative to this component because the launcher changes directory. The Open JSON button also switches books. A failed open retains the current book. With no input argument, the app opens its last successfully opened JSON path, or the demo on first launch.

Select a passage and press Space or Play selection. Space pauses or continues; Escape returns to the last displayed piece. Left/Right and Previous/Next browse conventional pages. A−/A+ changes font size. Resize and typography changes retain the source anchor. The WPM control starts at 350 and is saved separately for each book. Images, unsupported pieces and logical chapter transitions pause playback. Continue advances past an inspected image. Notes open on click and are skipped in playback by default. Enable Play footnotes in the contents sidebar to read them automatically and return after the reference. Escape returns to the reference in the book. Ornaments are skipped according to the producer's classification.

## Input and source locations

The input follows `CONTRACT.md` in the overnight workspace. `fixtures/demo.json` is a concrete version-1 example. Required fields include `book_id`, `revision`, `title`, `documents[].blocks[]`, and `pieces[]`. Blocks support heading, paragraph, list_item, image, ornament, unsupported and note. Runs preserve their text and strong/emphasis flags. Word pieces require source spans mapping to rendered runs. IDs, references and source ranges are validated before replacing the current book.

Source offsets count Unicode scalars. The Rust adapter maps them to UTF-16 positions used by QTextDocument and QML TextEdit, including words spanning several formatted runs and non-BMP characters. Resume stores revision plus all source spans, not a page number or piece index. Qt plain-text conversions for carriage returns, paragraph/line separators and nonbreaking spaces preserve their UTF-16 lengths. Empty text blocks follow Qt's omission behavior.

Image resources can be relative local paths beside the JSON, absolute paths or local `file:` URLs. Remote image URLs are rejected. Text is escaped before HTML generation; publication HTML, scripts and CSS are never executed. Images retain their intrinsic proportions and are only reduced to fit the reading area, including after reflow. Alt text stays in image metadata; missing resources without a URI show a text fallback. Unsupported blocks have visible labels. Core embedded PNG/JPEG/WebP resources resolve by resource ID. Inline links open detached core notes in a manual dialog; they are kept outside the main playback stream. Optional note playback is still pending in Qt. Legacy `kind: "note"` blocks remain supported.

State is isolated under `$XDG_STATE_HOME/overnight-qt-reader`, falling back to `~/.local/state/overnight-qt-reader`. Override it with `--state-dir DIRECTORY` or `READER_STATE_DIR`. No existing prototype storage is used. Saved JSON holds speed and the shared Rust source anchor, including up to twelve surrounding words and an identified chapter. It is written with owner-only permissions on Unix. The last-book file holds its local path. Reopening is always stopped. Changed editions use the core rule: at least eight exact words and 32 scalars occurring once, or a chapter with matching stable ID and title, for confirmation. If neither is found, the original state remains until the reader explicitly confirms a selected replacement location. This is an exact matching heuristic, not fuzzy or semantic recovery. Legacy state without matching context stays protected.

## Live theme proof

By default the adapter follows `~/.local/state/omarchy/current/theme/colors.toml`, watching the file and ancestors and rearming after replacement. It retains Omawrite's color precedence and desktop portal fallback. Portal changes update the fallback color scheme and text scale; Qt style hints are the next fallback. No desktop setting is written.

Use an isolated copy to test live changes:

```sh
cd prototypes/overnight/qt
mkdir -p .local
cp fixtures/theme-light.toml .local/colors.toml
READER_THEME_FILE="$PWD/.local/colors.toml" ./run.sh --demo
# In another terminal, from this component directory:
cp fixtures/theme-dark.toml .local/colors.new
mv .local/colors.new .local/colors.toml
```

## Verification

From this component directory:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings
cargo test --workspace --locked -j 2
cargo test --workspace --doc --locked -j 2
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked -j 2
./tests/theme.sh
./tests/whitespace.sh
./tests/smoke.sh
READER_TEST_PLATFORM=wayland ./tests/smoke.sh
cargo build --release --locked -j 2
READER_BINARY=target/release/overnight-qt-reader ./tests/smoke.sh
python tests/measure.py > evidence/measurements.json
```

The smoke path uses real Qt with an offscreen software backend by default, an isolated state directory and the synthetic book. It checks the rendered document against Rust's source mapping, native character formats, page navigation, Unicode selection, playback return, invalid opens, unclipped text lines, reflow visibility, actual timer-driven playback and stopped resume in a new process. It also checks recovery across changed source addresses and byte-for-byte preservation of the old saved state before confirmation. It writes synthetic screenshots to `evidence/reader.png`, `evidence/reader-playback.png`, and `evidence/reader-content.png`; runtime logs and state stay ignored. Committed captures from the verified offscreen and Wayland runs are in `evidence/`. The desktop smoke path briefly opens the synthetic app on the Wayland session. It is not a human usability or screen-reader assessment.

The measurement script samples the process tree and loaded libraries. File sizes, RSS and PSS are different quantities. Its first-frame timer begins at native application entry; it excludes process creation and dynamic loader work. Concurrent overnight workloads are uncontrolled. These measurements do not establish an advantage over another frontend.

See `REPORT.md` for actual results and remaining limitations. This is a renderer/binding proof, not a complete implementation of every accepted product decision.

The reading view defaults to Omawrite's IBM Plex Mono at 20 scaled pixels, a roughly 65-character column, and a quiet footer. The ellipsis reveals file, font and speed controls.

The model links `reader-core` for playback transitions, punctuation pacing, recovery, and atomic state writes. Legacy unchanged-edition anchors migrate through unique source matches. Qt still owns its timers, rendering, and recovery confirmation controls. The bottom-left three dots toggle the existing controls and a contents sidebar. Chapter selection stops playback and highlights the chapter start. The sidebar also offers optional footnote playback, chapter feedback, and per-book feedback history. Feedback records pace and understanding, with an optional explicit 25 WPM adjustment. Changed revisions offer the saved chapter start for confirmation without passage matching; missing chapters preserve the old saved location.
