# Source-aware reading core

Standalone Rust library and CLI for the overnight prototype. It exports contract version 1 JSON for both frontends. All sample prose and raster assets are synthetic. Private imports and saved locations belong in ignored `scratch/`.

From the repository root, one command builds and exports the synthetic demo:

```sh
prototypes/overnight/core/run.sh
```

The output is `prototypes/overnight/core/scratch/demo.json`. The script changes into this component, uses the committed lockfile and limits compilation to two jobs. No server or public listener runs. Rust 1.99.0 is pinned; the first build may download user-local dependencies.

From this directory:

```sh
./run.sh export --epub /path/to/book.epub --output scratch/book.json
./run.sh check --epub fixtures/complex.epub
./run.sh rbook --epub fixtures/legacy.epub
./run.sh export --epub fixtures/complex.epub --output scratch/split.json
./run.sh save --epub fixtures/complex.epub --piece b1:w1:0
./run.sh restore --epub fixtures/complex.epub
```

`check` reopens the original XML and verifies exported runs and pieces against decoded source ranges. It prints counts, never book prose. `save` selects a current piece ID, then persists its source spans rather than trusting that generated ID on reopening. `restore` prints exact restoration or a proposal requiring confirmation. The default store is this component's `scratch/store`; `--store PATH` selects another isolated store. Do not use another prototype's store.

## Integration

The crate exposes `import_epub(path, local_id, options) -> Result<Book>`, `verify_sources`, `inspect_rbook`, `location`, `playback`, and `session`. Imports either return a complete normalized book or an error; they do not replace caller state. The CLI writes JSON only after import succeeds.

`Book` retains document blocks and styled runs for conventional reading, plus source-linked pieces for playback. A word can cross several inline text nodes. Runs preserve decoded whitespace; reader rendering uses normal whitespace collapsing. Unicode word segmentation happens after joining inline runs. Adjacent lexical segments joined by hyphens/apostrophes form one lexical unit. Punctuation and non-word symbols remain attached to their surrounding display passage. This is Unicode segmentation with a compound policy, not morphological analysis.

Hyphenated compounds automatically split into playback beats, for example `state-`, `of-`, `the-`, `art`. ASCII hyphens and Unicode hyphen/nonbreaking-hyphen characters split between letters or digits in a token containing letters. Dates, numeric ranges, URLs, email addresses, leading/trailing hyphens and doubled dashes are not split. Hyphens remain attached to the preceding beat. The beats share one `word_id`, preserve exact source spans and leave the conventional reading text unchanged. Internal beats use 80% of the normal word duration; the final beat retains normal punctuation and paragraph pauses. Existing cached books receive the same idempotent conversion on load, with saved source locations preserved. The former per-compound `--split` option is no longer needed.

Source spans address archive document, XML child-node path, scalar offset and scalar length. Paths count all XML child nodes from the document root. Offsets count decoded Unicode scalar values, not UTF-8 bytes, UTF-16 code units or grapheme clusters. Use the book revision with each address. `playback::piece_at_source` maps a selected scalar to its unique main/note piece. `verify_sources` independently reparses XML; it does not compare the export with itself.

Image and unsupported pieces are interruption events. Raster `resource` paths resolve through `Book.resources`, which contains bounded data URLs. Do not load publisher URLs or scripts. SVG, tables, math, media and forms become explicit unsupported blocks. Missing images also become placeholders. The library preserves headings, semantic lists, emphasis and strong text. CSS font sizes do not invent heading semantics.

Note reference runs have additive `note_id` fields. The main piece stream contains a manual note event at that position. Note bodies live in `Book.notes`, outside main playback. `playback::main_plan` skips manual references and proven ornaments, emits image/unsupported pauses, and emits boundaries before subsequent logical chapters. `visit_note` returns a note plan and the exact immediate main return piece. Notes do not recurse. Final `End` is distinct from a transition to another chapter; a frontend owns the final chapter feedback UI. No pacing timer or renderer is supplied here.

Chapter IDs are resolved local destinations including fragments, not spine indices. A chapter can begin inside one file and continue into another. Inline fragment targets resolve to their whole multi-source word. Missing, hidden and note-only targets do not acquire later unrelated text. Duplicate or empty XHTML IDs produce diagnostics and structured warnings without blocking import or rewriting the source. Fragment navigation for duplicate IDs remains ambiguous. Explicit front-matter/part semantics and a small exact English front-matter label list exclude title/cover/contents pages from chapter feedback while retaining their text. Other navigation entries define chapter ranges in reading order. Nested navigation entries currently have the same granularity as top-level entries; no general semantic chapter inference is claimed.

## Identity and restoration

`LocalStore` assigns a random local UUID to a canonical input path. Publisher identifiers are metadata only. Different paths receive different identities even with identical publisher IDs or bytes. Symlink aliases share an identity. Replacing a file at the same canonical path retains its local identity; SHA-256 of the complete EPUB identifies its revision. A rename currently creates a new identity. Moving/merging library entries is outside this prototype.

Unchanged revisions restore exact source spans, including inline nodes. Changing approved display splitting can map the same source scalar to its containing display piece. Changed revisions never move automatically. A unique exact context of at least eight displayed words and 32 scalars produces a passage proposal. Otherwise matching stable chapter destination and title produces a chapter-start proposal. Repeated/no matches preserve the old saved location. These thresholds are conservative prototype heuristics, not measured semantic confidence. A changed splitting preference may reduce passage matching to chapter recovery.

Both native frontends link this crate. `session` supplies playback transitions, interruption kinds, punctuation/paragraph delays, the 60–1200 WPM range, resume decisions, and source-anchor capture. Qt calls it directly. Tauri obtains compiled action tables over IPC when opening a book and asks Rust to capture the current anchor when saving. Timers, rendering, confirmation dialogs, and feedback controls remain in each shell. The standalone browser preview still uses its JavaScript compatibility model; it is not evidence for native policy parity.

Native records retain per-book preferences and store a core `SavedLocation` under `location`. Legacy frontend records migrate through a unique exact source match in an unchanged edition. Old short contexts never qualify for changed-edition passage recovery. A legacy chapter fallback requires both the saved stable ID and title; old Tauri records without a title stay unresolved. Recovery proposals do not write state, and reopening remains stopped.

Both shells use per-book checkpoints capped at 1 MiB. Writes use a unique temporary sibling, sync file contents, atomically replace the destination, and sync containing directories on Unix. Unix files are created with mode 0600 and new state directories with mode 0700. Recovery tries the current record, the previous valid record, then a separate chapter-start record. Damaged records remain available for diagnosis. Legacy whole-library records migrate without overwriting other books.

State directories remain separate. Both shells hold a shared per-user OS lock for the open book identity, including when their scratch directories differ. Different books can open concurrently. Process death releases the lock. Canonical-path identity allocation is also locked to prevent concurrent first imports from generating different identities. Reopening remains stopped. Playback clocks and note excursions are not persisted.

## Verse typography and XML

Verse ignores publisher CSS, including inline styles, embedded style elements, linked stylesheets, fonts, media queries and Adobe page templates. Missing or malformed styling resources do not block import. Headings and inline emphasis come from HTML tags. HTML `hidden` attributes suppress descendants; text hidden only by publisher CSS remains readable. Scripts, styles, templates and document heads never enter playback.

XML DOCTYPE declarations and bounded internal entities are accepted. External entities and external DTDs are never fetched. XML parse errors retain the underlying parser explanation. Archive, XML node/depth and text-size limits still apply. XHTML must be well-formed UTF-8 XML; HTML error recovery, remote resources, fixed-layout books and DRM remain unsupported.

## Ornament evidence

The importer inspects every supported raster resource across the archive and counts references across main text and notes. It decodes bounded RGBA pixels and hashes original, horizontal, vertical and both reflections to identify exact repeated/mirrored assets. It does not establish approximate visual similarity or inspect SVG ornament content.

Repetition, small dimensions and empty alt text alone never justify skipping. An asset is an ornament only when every encountered reference explicitly declares `role=presentation` or `role=none`, has empty alt text, and the image is small and not possibly animated. Ambiguous assets stay substantive and pause. GIF/WebP and suspected APNG retain substantive treatment because only the first frame is analyzed. Reports include the evidence and reason per asset. No claim of human visual classification is made.

## Bounds and verification

Defaults cap compressed input at 64 MiB, 4096 ZIP entries, 16 MiB per expanded entry, 128 MiB actual total expansion, 500,000 nodes per XML document, depth 128, 128 million publication text scalars and 40 million pixels per image. There is no cumulative image-pixel budget. ZIP reads enforce actual expansion and reject mismatched size metadata, duplicate entries and unsafe archive paths. Bounded internal XML entities are accepted; external entities are never resolved. Image decoder allocation limits are best-effort; per-image dimensions are checked explicitly. These bounds are not an OS sandbox or a proven execution-time ceiling.

JPEG, PNG, WebP, GIF, AVIF, BMP and TIFF are decoded. GIF, AVIF, BMP and TIFF are exported as PNG for consistent rendering in both frontends. GIF/TIFF/AVIF inspection and conversion use the first image/frame. Building AVIF support requires `pkg-config` and libdav1d development files.

## Import logs

The CLI uses `tracing` and `tracing-subscriber` to emit JSON events to stderr. Import start/end events and warnings are enabled by default; `RUST_LOG=reader_core=warn` selects warnings and errors. A duplicate-ID warning records `code`, `epub`, `document` and `id`. The EPUB and its IDs are never rewritten. Diagnostics also remain in the exported book JSON. To save importer logs separately from the export, run `reader-core export --epub book.epub --output book.json 2>import.jsonl`. Malformed XML/HTML reports the internal document path and parser position; damaged ZIP errors identify the missing archive directory or other underlying failure.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings
cargo test --workspace --locked -j 2
cargo test --workspace --doc --locked -j 2
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked -j 2
cargo build --locked -j 2
```

See [REPORT.md](REPORT.md) for executed results and [fixtures/README.md](fixtures/README.md) for independent expected text and source behavior.

The `experience` module owns preparation/ramp constants and the native timing policy. `vocabulary` launches the shared Python worker in `vocabulary/`, reusing the accepted capture, asynchronous ranking, SQLite storage and offline dictionary implementation. The worker maps normalized source spans back to paragraph coordinates, preserving split-compound coverage and sentence context. Native adapters record actual main-text exposure, excluding preparation and pauses. See `../integration/README.md` for runtime configuration and checks.

## Disk-backed native reading

CLI `export` and `demo`, used by the native launch scripts, stage one XHTML document at a time beside the output file. ZIP validation and revision hashing use fixed buffers. The aggregate `import_epub` API remains available for diagnostic and source-verification commands.

`book_store::DiskBook` streams normalized JSON into indexed files under `XDG_CACHE_HOME/quiet-reader/book-spools`, with `HOME/.cache` as fallback. A reusable text buffer reads the current 512-piece window with 128 pieces of backward overlap. Both native renderers replace their old text, source maps and playback plans when changing windows. Vocabulary replaces its active text window while retaining captured evidence in SQLite. OS leases protect live spools; subsequent opens remove abandoned spools after a crash.

Image resources are separate from text windows. The working set contains at most two preceding images, the current image when selected, and two following images. Images are prepared at a maximum 1600-pixel edge. Qt decodes through a single asynchronous worker; Tauri predecodes the nearby images and drops old image references.

Bounds apply to bytes as well as piece counts: 16 KiB per serialized piece, 2 MiB of visible context, and 1 MiB of aggregate note records per window. Notes exceeding that budget show an explicit unsupported-content marker. Import still holds one XHTML DOM at a time, bounded by the 16 MiB expanded-entry limit. A normalized JSON block or resource is also decoded as a unit during indexing. These are import-time bounds, not a claim of constant memory regardless of document structure.

See `../integration/RELIABILITY.md` for verification and remaining limitations.
