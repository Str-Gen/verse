# Verse development builds

From the repository root, build the current applications and expose them on PATH:

```sh
./build.sh
verse book.epub
verse-qt book.epub
verse-tauri book.epub
verse --demo
verse --book-json export.json
verse-tauri --state-dir /tmp/verse-trial book.epub
```

`verse` selects Qt. The build defaults to release and creates symlinks in `~/.local/bin` to checkout-local launchers. Those launchers use the already built core importer and frontend; they never build on launch. Relative input paths are resolved from the caller's directory. Paths containing spaces work when quoted. With no input, the frontend reopens its last book. The EPUB cache, canonical-path identities, and frontend state locations are shared with the older development launchers, so this does not create a separate reading library.

Build options: `--debug`, `--frontend qt|tauri|both`, `--name NAME`, `--bin-dir PATH`, and `--no-link`. The build refuses to overwrite an unrelated command. It prints a PATH reminder if the chosen directory is absent from PATH. After moving the checkout, remove the old command symlinks and rebuild. To remove the commands, remove the symlinks from the chosen command directory; the build artifacts and reading state remain in place.

For the existing build-and-run development workflow:

```sh
./prototypes/overnight/qt/run.sh --demo
./prototypes/overnight/tauri/run.sh --demo
```

Both commands build the same pinned Rust core, export its synthetic EPUB, build the selected frontend, and launch it. Use `--epub /absolute/path/book.epub` to import a local DRM-free EPUB or `--book-json /absolute/path/export.json` to open an existing export. Input paths are resolved before changing directories. With no arguments the frontend retains its own last-book behavior. Run only one instance per frontend storage directory.

`export.sh` exposes the core CLI unchanged. `launch.sh` owns import and the persistent canonical-path identity store in `.local/identities`. Exports live under `.local` with a filename derived from the EPUB path. Replacing the bytes at that path retains book identity while changing revision. The two renderer projects remain separate Cargo projects; there is one EPUB importer and reusable Rust library in `../core`. Both native frontends link the shared Rust application policy for playback transitions, recovery, and source-anchor persistence. EPUB import still uses the versioned JSON process boundary.

## Verification

```sh
cargo +1.99.0 test --manifest-path prototypes/overnight/core/Cargo.toml --locked -j 2
python prototypes/overnight/integration/check_recovery.py
python -m unittest discover -s prototypes/overnight/integration/tests
./prototypes/overnight/qt/tests/check.sh
./prototypes/overnight/integration/check_native.sh
```

The native script requires the installed Wayland session, Qt 6 and WebKitGTK. It tests real Qt offscreen and Wayland renderers, then runs Tauri's real WebKitGTK suite twice. It generates fresh synthetic snapshots in the Tauri web directory before building. Qt's inherited fixture suite also exercises page layout, timer-driven playback, failed open, per-book persistence and confirmation across restarted processes. Its theme suite uses a synthetic override and tests real Qt file watching, including atomic replacement and initially missing ancestors. No desktop theme is modified.

`check_recovery.py` calls the actual CLI/store against the committed synthetic editions in `fixtures/` and checks that proposals never modify saved state. The core's eight-word confidence threshold sends the short unique fixture to chapter confirmation. Both native frontends now use this same conservative recovery policy. The standalone browser preview retains its JavaScript compatibility model.

The standalone normalized Unicode fixture exercises native source selection and return in both apps. Core Rust tests cover EPUB import policy, source spans, image normalization and ornaments. Browser tests, mobile ports, broad publication compatibility and user visual acceptance are not implied by native tests.

For sequential resource samples, build both release artifacts, wait until no builds remain, then run `python prototypes/overnight/integration/measure.py`. It refuses to start while build processes exist. Outputs and synthetic screenshots are in ignored `evidence/`; the final control-directory archive preserves them. See the root `MORNING-REPORT.md` for observed outcomes and unresolved gates.

## Reading experience integration, 21 October

Both native apps now use a two-second preparation pulse and a 30-second normalized tanh ramp from half to full selected WPM. A pause of up to 30 seconds after reading begins resumes at full speed. Returning to conventional reading, selecting a new passage, or reopening a book starts fresh. Cancelling preparation does not count as reading. Punctuation and paragraph timing scale with the ramp.

Press **C** during playback to capture the last five seconds of actual display time. The controls also offer a three-sentence window. Press **V**, or choose **Word list**, for book-specific review. A capture saves locally before optional Jev ranking; unavailable credentials and failed requests remain visible and retryable. Each ranked tap offers one suggestion with its source sentence, unfamiliar / slow recognition / dismiss choices, and expandable local definitions. Saved words have voluntary recognition practice at the selected WPM, with context/definition reveal and temporary Comfortable / Difficult / Skip counts. Preparation, pauses, and note excursions do not enter the main-text capture history. The capture confirmation reminds the reader that the word list is stored for later review.

The accepted prototype's vocabulary and dictionary modules now live under `core/vocabulary`. Both Rust shells call the same private Python JSON-lines worker; Python 3 is a runtime dependency for vocabulary. No local HTTP service is needed. Each app keeps `vocabulary.sqlite` in its own state directory, alongside its existing reading state. The earlier prototype database is unchanged. Dictionary lookup stays offline and retains all supplied senses and attribution.

Configuration:

- `TYPESAFE_API_KEY` or `VERSE_KEY_FILE` supplies ranking credentials. On this development machine, the existing `~/speedreading/jev-speedreading.key` is the default key file. Only explicit capture/retry actions request ranking.
- `VERSE_DICTIONARY` selects the local Kaikki SQLite file. The development default is the existing `~/speedreading/omawrite/test-data/local/dictionary.sqlite`. A missing installation and a missing entry have distinct messages.
- No ranking service was called during verification. Tests unset the key and use synthetic dictionary data or the offline lookup path.

Verification adds `python -m unittest discover -s prototypes/overnight/core/vocabulary -p 'test_*.py'`, `qt/tests/experience.sh`, and Tauri's native capture/practice checks to the existing suites. Qt smoke tests now wait for actual preparation completion before asserting playback transitions.

Images already normalized GIF, AVIF, BMP and TIFF to PNG before this work. Both frontend allowlists also accept embedded GIF from older/external exports now. The core's seven-format decode/export and conservative ornament tests pass. Standalone SVG remains an explicit unsupported pause and is deferred. Broad visual ornament recognition is not claimed.


The legacy independent export oracle, source-path adapter, unused UI trace framework and stale CSS/SVG expectations have been removed. Recovery fixtures and process measurement remain self-contained here. Historical reports describe the earlier runs; their deleted commands are not current verification instructions.
