# Verse

Verse is a reading and learning app with Qt and Tauri frontends and a shared Rust EPUB core. It supports conventional reading, paced word playback, source-based resume, and local vocabulary review.

This is an independent source repository. Linux development builds are supported; installable releases, accessibility acceptance, Windows and macOS validation are still pending.

## Build and run

Install Rust 1.99.0, Python 3, a C++ toolchain and the native dependencies for your frontend. Qt needs Qt 6 development tools and Core, Gui, Qml, Quick, QuickControls2, Concurrent, DBus and Quick Dialogs. Tauri needs its Linux GTK/WebKitGTK development dependencies. The core's AVIF decoder also needs the native dav1d development library and pkg-config.

```sh
./build.sh --no-link                 # build both release versions
./build.sh --frontend qt             # build Qt and create PATH commands
verse "path/to/book.epub"
verse-tauri "path/to/book.epub"       # after building Tauri with command links
```

The default command directory is `~/.local/bin`. The build refuses to overwrite commands belonging to another checkout. Use `--bin-dir PATH` for an isolated installation, `--debug` for development, or `--no-link` to build without changing PATH commands. Launchers depend on this checkout and do not compile on launch. After source changes, run `./build.sh --frontend qt --no-link` to refresh the release binary used by `verse` and `verse-qt`. A plain `cargo build` only updates the debug binary.

Use `--demo` for the synthetic EPUB, `--book-json PATH` for a normalized export, and `--state-dir PATH` for isolated reading state. With no input, each frontend reopens its last book.

Vocabulary capture works locally. Optional ranking requires `TYPESAFE_API_KEY` or an explicit `VERSE_KEY_FILE`. Offline definitions come pre-filled from the bundled Wiktionary/Kaikki dictionary. On the first definition lookup, Verse verifies and expands `assets/dictionary/english.sqlite.gz` into the per-user Verse cache. No download is needed. `VERSE_DICTIONARY` overrides that database; `VERSE_RESOURCE_DIR` selects an installed assets directory. Credentials never ship in the repository. See [dictionary attribution](assets/dictionary/DICTIONARY-NOTICE.md) and [dataset metadata](assets/dictionary/manifest.json).

## Cargo workspace

The root `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml` define one workspace. Cargo writes builds to `target/`.

- `crates/reader-core` contains EPUB import, playback, recovery, persistence and the Python vocabulary worker.
- `crates/reader-cli` provides the `reader-core` importer command and its logging setup.
- `apps/qt` contains the `verse-qt` Qt/QML application.
- `apps/tauri/src-tauri` contains `verse-tauri`; its web assets live in `apps/tauri/web`.
- `assets` contains redistributable fonts and the compressed English dictionary with licenses.
- `scripts` contains development launchers, integration tests and measurement tools.
- `docs/private` is ignored. Keep investigations, benchmark results, profiles, captures and private build experiments there. Existing historical reports have been moved there locally.

```sh
cargo build --locked                         # core and CLI; no GUI toolkit
cargo test --locked                          # core and CLI tests
cargo build -p verse-qt --locked              # Qt application
cargo build -p verse-tauri --locked           # Tauri application
cargo build --workspace --release --locked   # all production binaries
```

GUI crates are explicit workspace members, excluded from `default-members`. Tests use `dev-dependencies`; native code generators use `build-dependencies`. Cargo's resolver 3 keeps development/build feature resolution separate from ordinary target dependencies. See [Cargo dependency kinds](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html) and [feature resolution](https://doc.rust-lang.org/cargo/reference/resolver.html).

| Dependency | Scope | Reason |
| --- | --- | --- |
| `rbook` | Core `dev-dependencies` | Independent EPUB parser comparison in contract tests only. The old evaluation-only CLI command is removed. |
| `tracing-subscriber` | CLI only | JSON log setup; the reusable core and GUI crates do not initialize it. |
| `cxx-qt-build` | Qt `build-dependencies` | Generates native bindings and QML resources. |
| `tauri-build` | Tauri `build-dependencies` | Generates application build metadata/resources. |
| `cxx`, `cxx-qt`, `cxx-qt-lib` | Qt only | Required at runtime by the native frontend. |
| `tauri` | Tauri only | Required by the webview frontend. |
| `image` / dav1d, ZIP, XML, hashing, serialization | Core runtime | EPUB import and reading use these; moving them to test dependencies would break production. |

The dictionary is an application resource, not embedded in Rust binaries. The checked-in gzip is about 95 MiB and expands to about 397 MiB with 1,492,836 English entries. SQLite lookup uses read-only connections. The original multilingual raw dump and personal vocabulary database are not included.

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests
python3 -m unittest discover -s crates/reader-core/vocabulary -p 'test_*.py'
node --test apps/tauri/tests/model.test.mjs apps/tauri/tests/comfort.test.mjs
./apps/qt/tests/check.sh
./scripts/check_native.sh
```

The native integration checks require Linux with Qt, WebKitGTK and a Wayland session. They do not establish accessibility or support for other operating systems. See the [development guide](scripts/README.md) and [reliability checks](scripts/RELIABILITY.md).

`.gitignore` excludes private work, generated builds, local databases, environment files and key files. It does not erase material from earlier commits; inherited research remains in Git history. Never force-add credentials or `docs/private`.

## Packaging review

The removed `pkgbuild/` packaged the Omawrite editor. It never packaged Verse. `build.sh` currently produces checkout-local developer commands, and Tauri's bundle setting is disabled.

Before producing installers:

1. Resolve the Python vocabulary worker and demo assets from installed resources. The binaries currently embed build-source paths through `CARGO_MANIFEST_DIR`; copying the executables alone breaks those paths.
2. Move import caches, identity records and Tauri's default reading state out of the source tree into per-user platform directories. Preserve existing book identities and saved positions during migration. Qt's existing state directory also needs a migration plan before renaming it.
3. Choose stable application identifiers and installed binary names. Tauri still uses `local.overnight.quietreader`; the GUI binaries are now `verse-qt` and `verse-tauri`. Keep storage identifiers unchanged until migration is defined.
4. Package the core importer, selected frontend, Python worker and its runtime, synthetic fallback assets, notices, and native runtime dependencies. Include the bundled dictionary archive, manifest and attribution in every distribution that promises offline definitions. An executable-size measurement is not an installer-size measurement.
5. Add desktop integration and build installers. Test installation, launch from a different directory, a read-only install location, upgrade with existing state, and uninstall without deleting reading data.

For Linux, start with a package that declares system Qt/WebKitGTK, dav1d and Python dependencies. A portable bundle requires a separate dependency and plugin inventory. For macOS and Windows, first make native builds pass, then test their installers on those systems. Signing and notarization follow working native packages.

### Portability gates

Qt currently links D-Bus unconditionally and its theme adapter includes Linux portal code. Split that adapter by platform before claiming Windows or macOS support. Development launchers use POSIX shells, `realpath`, symlinks and executable names without `.exe`; process and locking tests also contain Linux-specific assumptions.

Run core and vocabulary tests on Linux, macOS and Windows, then build and exercise both native frontends on each claimed platform. Cover file dialogs, Unicode paths, image decoding, simultaneous book access, crash recovery and upgrades. No non-Linux result is claimed here.

### Accessibility gates

Static inspection finds accessible names on some QML controls and ARIA labels on some web controls. That is not a completed accessibility review.

Review both frontends with keyboard-only navigation and a screen reader on each supported platform. Check reading order, visible focus, dialog focus entry and return, control names and values, image alternatives, text scaling, dark/light contrast, and pause/return controls. Verify that rapid playback does not flood announcements and that conventional reading remains usable. Review preparation animations and reduced-motion behavior. Record actual assistive technology, operating system and outcomes before release.

## Origin and license

Verse began in a local clone of [Omawrite](https://github.com/omacom/omawrite). Its Git history is retained. The original Markdown editor and Arch packaging have been removed from the current tree. The IBM Plex Mono font files remain under their bundled SIL Open Font License in `assets/fonts/OFL.txt`; Qt currently requests the font by family name, so installation or explicit resource loading belongs in packaging. The Qt theme adapter still derives from Omawrite, and its copyright notice and the [MIT license](LICENSE) are retained.

This checkout has its own Git object database and no remote configured. It does not depend on the original repository or its worktrees to build. The old worktrees and their local reading data remain untouched. Existing PATH commands still point to those checkouts until explicitly replaced; this separation does not migrate their state.
