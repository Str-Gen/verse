# Core worker report

Completed a runnable standalone Rust library and CLI in `prototypes/overnight/core`. Contract version 1 exports are available through `reader-core export --epub PATH --output PATH` and `reader-core demo --output PATH`. Run `prototypes/overnight/core/run.sh` from the repository root for the synthetic demo. It writes ignored `scratch/demo.json` and uses two compilation jobs.

The core joins words across inline formatting, preserves multiple scalar source spans, applies one supported CSS visibility interpretation to both output views, resolves logical chapter fragments independently of spine files, separates local identity from publisher identifiers and revision hashes, and proposes rather than silently applies changed-edition recovery. It includes conservative raster evidence, manual note plans with return positions, source-click lookup, malformed-input bounds, fixtures and regression tests.

## Executed verification

Commands below ran from `prototypes/overnight/core` with Rust/Cargo 1.99.0 on this Linux host. The project pins that toolchain and commits Cargo.lock. Builds were local debug builds; no production release or push occurred.

| Command | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed after formatting the final edits. |
| `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` | Passed with no warnings. |
| `cargo test --workspace --locked -j 2` | 18 module tests and 13 integration tests passed; zero failures. |
| `cargo test --workspace --doc --locked -j 2` | Passed; there are zero doctests. |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked -j 2` | Documentation built successfully. |
| `cargo build --locked -j 2` | CLI and library built successfully. |
| `./run.sh` | Exported synthetic schema 1 JSON: 2 main documents, 48 main pieces, 3 chapters, 1 manual note. |
| `./run.sh check --epub fixtures/complex.epub` | 53 main/note pieces verified against decoded source; 4 raster assets, 2 assets in an exact mirrored group, no inferred ornaments, 1 broken-reference diagnostic. |
| `./run.sh save --epub fixtures/complex.epub --piece b1:w1:0` followed by `./run.sh restore --epub fixtures/complex.epub` | Saved and restored exactly to `b1:w1:0`. This word crosses three inline source nodes. |
| `./run.sh check --epub /home/strgenix/speedreading/omawrite/test-data/local/default.epub` | Passed; counts below. No private prose printed. |
| `./run.sh rbook --epub /home/strgenix/speedreading/omawrite/test-data/local/default.epub` | rbook 0.7.12 opened the bounded archive and reported 84 spine entries. |
| `python3 fixtures/generate.py` plus SHA-256 comparison and `zipfile.testzip()` | Valid synthetic fixtures regenerate byte-identically with valid ZIP CRCs and first, uncompressed EPUB mimetype. Deliberately malformed fixtures remain malformed. |

The 13 integration tests assert independently specified visible text and source spans. They cover EPUB2/EPUB3 parity, `naï<em>ve</em>té` and `co<strong>op</strong>erate`, combining Unicode scalars, hidden text and restored visible descendants, fragment destinations inside words, chapters spanning files, excluded title/hidden/note targets, duplicate IDs, print/screen media, broken notes, unsupported SVG, image ambiguity, publisher-ID isolation, persistent exact restore, approved display splits and changed-edition proposals. Malformed ZIP/XML, missing resources, unsafe archive paths and nine configurable resource bounds fail explicitly. A forged central-directory size with valid compressed data/CRC cannot bypass the actual expansion limit.

Module tests additionally exercise cascade specificity and `!important`, exact multi-source restore, ambiguous/no-match recovery preserving the old location, corrupt/oversized persistence, explicit decoration evidence, pixel limits, manual note return and non-recursive note behavior.

## Private-book evidence

The private run verified 186,861 main/note pieces across 84 main documents against decoded source XML. It exported 75 resolved navigation chapter ranges after front-matter filtering, zero semantic notes, six raster assets with 84 encountered references, and one unsupported-content event. No assets qualified as proven ornaments; no distinct assets matched under exact pixel reflections. The check reported zero importer diagnostics. The unsupported event remains explicit in the stream; diagnostics are not the only indication of unsupported content.

These are measured importer counts, not a claim that a human reviewed every chapter or image. Source verification proves that exported text corresponds to its source; it does not prove exhaustive visual fidelity or correct interpretation of every publisher convention. The classifier inspected pixels computationally across resources; it did not make a human visual judgment. Repeated small images remain substantive because the book did not provide sufficient explicit decoration evidence under this policy. No private prose, images or generated dictionary data are committed.

## rbook evaluation and architecture decision

The current primary [rbook documentation](https://docs.rs/rbook/latest/rbook/) describes EPUB2/EPUB3 package, metadata, manifest, spine, navigation and resource access, including lazy resource reads. `cargo info rbook` retrieved version 0.7.12, and its downloaded crate documentation/source supplied the exact API used in the local build. The probe compiles against the pinned version with default features disabled and calls `Epub::read` only after our ZIP preflight. Integration tests opened both synthetic EPUB versions and observed three spine entries, including the non-linear note resource. The private probe observed 84 entries.

rbook is useful as a package/resource candidate. The tested API does not itself establish the contract's CSS visibility, joined lexical words, scalar source correspondence, application identity or recovery semantics. This prototype therefore retains it as an executable evaluation command while the normalizer uses bounded ZIP reads and roxmltree to preserve decoded XML node identity. It does not run rbook and then trust rewritten HTML for saved positions. A future adoption can replace package/navigation plumbing behind the public import API; that choice still needs broader corpus and dependency review. This is a local prototype decision, not closure of the planning map's production architecture ticket.

Primary references consulted include [EPUB 3.3](https://www.w3.org/TR/epub-33/), [rbook's repository](https://github.com/DevinSterling/rbook), [cssparser](https://docs.rs/cssparser/0.35.0/cssparser/), and [image decoder limits](https://docs.rs/image/latest/image/struct.Limits.html). Local crate sources were inspected for actual ZIP read behavior and XML parser bounds. The forged-size regression follows that inspection rather than assuming ZIP metadata is trustworthy.

## Boundaries and remaining work

- CSS is an explicit subset implemented with cssparser tokenization. Unsupported selectors/at-rules and selected content-changing properties reject import. Reader-controlled color, font size and layout are discarded; transparency, zero-size/offscreen placement and every browser visibility effect are not reproduced. The README defines the exact boundary. A complete CSS/layout engine remains future work.
- UTF-8 XML XHTML is required. Fixed-layout publications, arbitrary SVG, tables, math, media/forms, DRM, HTML error recovery and remote resources are not rendered. Unsupported blocks pause visibly. General hyperlinks remain readable text but are not exposed as a full conventional-view link API.
- Navigation entries define logical chapter ranges. Explicit front-matter semantics and exact common English labels are excluded; arbitrary multilingual front-matter and nested chapter/subsection hierarchy remain imperfect. No numeric spine fallback invents a chapter identity.
- Ornament detection uses exact decoded pixels and reflections, not perceptual or scaled-image similarity. Repetition never proves decoration. Image decoder allocation limits are best-effort; dimensions and total pixel counts are checked explicitly. No malicious-corpus fuzzing or OS memory/time sandbox was run.
- Identity is local canonical-path identity. Renames create a new local identity. Persistence assumes one writer per publication, uses bounded private JSON and atomic replacement, and does not implement process locking. Changed-edition unique context thresholds are heuristics. A split-display preference change may reduce a passage proposal to a chapter proposal.
- The core does not persist note excursions, per-book WPM, feedback history or timing state. It supplies content, source location and interruption APIs for the frontend/application layer. No Qt/Tauri GUI, production packaging, other operating system, dependency advisory/license audit, or broad external EPUB corpus was validated here.
- No footprint/startup comparison is claimed. Concurrent overnight builds would not support such a comparison.

All source, manifests, lockfile, synthetic fixtures and documentation are confined to the assigned component. Existing prototype references were read without modification. The planning map and other workers' files were not edited. Scratch state and build output are ignored locally. No required core gate remains blocked.
