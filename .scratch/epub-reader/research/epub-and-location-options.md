# EPUB libraries and reading locations

Research date: 2026-10-20. This is a research recommendation, not an architecture decision. No EPUB has been rendered or tested, including the book the user plans to supply.

## Answer

Rust has credible EPUB package libraries. Evaluate `rbook` first and keep `epub` as a comparison. The application still needs XHTML interpretation, conventional-view layout, and a shared location model. A crate that opens an EPUB does not provide a complete reading system.

Both views should address the same source passage. Build playback tokens with source spans, then restore the conventional view from the last displayed token's span. Treat this as a requirement to prove in a prototype before choosing the renderer.

## Library candidates

Versions and release dates below were checked against the crates.io API, because search results and docs.rs pages sometimes showed older releases.

| Candidate | Confirmed capability and maintenance evidence | Assessment |
| --- | --- | --- |
| `rbook` 0.7.12 | Released 2026-10-10; Apache-2.0; Rust 1.88 minimum. EPUB 2/3 package, metadata, manifest, spine, EPUB 2 NCX and EPUB 3 navigation. Lazy resource reads and configurable linear-content iteration. Writing can be disabled. | First candidate for a Rust content layer. Recent releases and detailed APIs are positive evidence, not a compatibility guarantee. |
| `epub` 2.1.5 | Released 2025-10-29; GPL-3.0. Archive resources, metadata, spine with linear flags, chapter navigation and raw chapter bytes/strings. | Useful comparison with a smaller API. The GPL dependency needs an explicit project licensing decision if distribution becomes a goal. No full EPUB-version conformance claim was verified. |
| `epub-parser` 0.3.4 | Released 2026-02-07; MIT. OPF/NCX parsing and cleaned plain-text pages in spine order. Its public `Page` stores only an index and text. | Less suitable as the main representation because stripped text lacks the source spans needed for exact return to XHTML. EPUB 3 navigation coverage remains unverified. |

Sources: [rbook release metadata](https://crates.io/api/v1/crates/rbook), [published rbook source](https://docs.rs/crate/rbook/0.7.12/source/), [rbook EPUB APIs](https://docs.rs/rbook/latest/rbook/epub/), [epub release metadata](https://crates.io/api/v1/crates/epub), [epub APIs](https://docs.rs/epub/latest/epub/), [epub-parser release metadata](https://crates.io/api/v1/crates/epub-parser), [epub-parser Page](https://docs.rs/epub-parser/0.3.4/epub_parser/types/struct.Page.html).

I inspected the downloaded published sources for all three versions. `rbook`'s current changelog records dependency updates and parser fixes; its default features include writing, so a reader can select only the needed features. Its documentation says multi-rendition books use the first rootfile. The source/API inspection did not find a ready-made CFI location engine in either shortlisted package library. These are bounded observations, not an exhaustive compatibility audit. [rbook source](https://docs.rs/crate/rbook/0.7.12/source/), [rbook rendition limitation](https://docs.rs/rbook/latest/rbook/epub/struct.Epub.html), [epub source](https://docs.rs/crate/epub/2.1.5/source/).

## Package parsing and displayed text are separate

EPUB packages contain structured web content. The spine defines the default document sequence; the table of contents is navigation, and need not enumerate every reading passage. `linear="no"` marks auxiliary content. EPUB permits UTF-8 and UTF-16 XML resources. [EPUB 3.3](https://www.w3.org/TR/epub-33/).

Recommendation: follow the spine, with an explicit policy for auxiliary content and footnotes. Do not sort ZIP filenames or walk the table of contents as the book's text sequence. `rbook` can select `LinearOnly`; its default is `Original`, which retains auxiliary entries. [LinearBehavior](https://docs.rs/rbook/latest/rbook/epub/reader/enum.LinearBehavior.html).

A browser renderer can interpret publisher XHTML/CSS, but its DOM must map back to the content used by playback. A native text renderer can consume a simplified document model, but the project then owns decisions about emphasis, headings, links, whitespace and unsupported layout. These are architecture alternatives, not established crate capabilities.

## Location recommendation

Use a versioned app location containing a book fingerprint, spine resource href, structural source anchor, explicit offset unit, and a short text quote with surrounding context. Store a progress fraction only as an approximate fallback. A source anchor may be a CFI or an app-defined node path. Avoid using window scroll pixels or a bare token number as the durable position.

CFI is the EPUB-specific option for structural locations and ranges. Its character offsets count UTF-16 code units, not UTF-8 bytes or grapheme clusters. Supporting CFI requires its actual path and text-node rules; an arbitrary DOM path should not be labelled CFI. CFI does not remove the need to map transformed or normalized text back to source. [EPUB CFI specification](https://w3c.github.io/epub-specs/epub33/epubcfi/).

Readium provides a useful alternative model with resource href, several location expressions, and before/highlight/after text context. Recommendation: borrow that separation even if full Readium compatibility is deferred. Text matching can be ambiguous, so recovery must detect ties rather than silently choose a repeated paragraph. [Readium locators](https://readium.org/architecture/models/locators/).

During extraction, retain source spans across inline markup, decoded entities and collapsed whitespace. A word can cross several text nodes, so one token may need several source ranges. Preserve paragraph and sentence boundaries separately for pacing. `unicode-segmentation` supplies Unicode word, sentence and grapheme boundaries with byte offsets; it does not itself provide language-specific pacing or source mapping. [unicode-segmentation](https://docs.rs/unicode-segmentation/latest/unicode_segmentation/).

Recommendation: track the currently displayed span separately from the next playback token. On stop, reveal the displayed passage; on resume, use an explicit tested rule for whether that token repeats. Reflow, font changes and resizing should change geometry while retaining the source location. Extraction-version changes should invalidate cached token indices and recover from the persisted source anchor.

## Validation needed before selection

- Open the supplied book locally, plus redistributable EPUB 2 and EPUB 3 fixtures. Check chapter order, NCX/navigation, links, nested resource paths and final chapter transitions. Do not commit the user's book.
- Demonstrate conventional view to playback to conventional view at a selected word, including across paragraph and chapter boundaries. Repeat after resizing, changing font size, closing and reopening.
- Test inline emphasis inside words, apostrophes, punctuation, nonbreaking spaces, soft hyphens, combining marks and non-BMP characters. Include UTF-16 resources. Verify no skipped or duplicated text and correct offset conversions.
- Decide how playback handles footnotes, hidden text, headings, lists, images and unsupported fixed-layout content. DOM text extraction alone does not establish what the conventional view actually shows.
- Check both candidate libraries against malformed resources, archive size limits and path handling. For a web renderer, disable book scripts and external requests and restrict resource access to the publication.
- Verify dependency builds for the actual Linux toolchain, and later cross-compile the isolated content layer for intended mobile targets. Library portability does not establish GUI portability.

The strongest prototype gate is exact passage round-tripping through both modes. Library choice should follow that evidence and the content-policy decision.
