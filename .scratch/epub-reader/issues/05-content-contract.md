# What EPUB content and reading-position behavior does the first release promise?

Id: 05
Parent: ../map.md
Labels: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Blocked by: 03

## Question

Set the first-release contract for chapters, styling, links and notes, images, unsupported content, errors, opening files, and remembered positions. Inspect the user-supplied EPUB when available. Choose deliberate degradation behavior and define resume semantics without silently excluding ordinary fiction/nonfiction content.

## Comments

2026-10-20, user decisions: Conventional reading uses consistent, reader-controlled typography like Omawrite, while retaining headings, italics, bold, lists, images, and notes. Speedreading pauses on images. Footnotes are available on demand rather than automatically included in playback.

2026-10-20, further user decisions:

- Pause at chapter boundaries and offer a small prompt about speed and the reader's perceived understanding of the words. Store the feedback for possible future guidance toward a more pleasant or appropriate pace. Allow skipping the prompt to continue and disabling it entirely. The interaction and guidance policy are tracked in [chapter-end feedback](10-chapter-feedback.md).
- Launch into the last book at the saved position in conventional view, with playback stopped.
- Do not follow footnote markers by default. Provide a toggle to follow them when encountered. After visiting a footnote, continue the main text after its inline marker.

2026-10-20, user confirmation: When footnote following is enabled, play the note through in speedreading mode, then automatically continue the main text after its inline marker.

2026-10-20, user confirmation: Include a chapter list and Open File action. Drag-and-drop is a nice-to-have. Remember each book's position when switching books. Show clear placeholders for unsupported content, pause playback there, and explain book-opening failures while keeping the current book available.

Inspected the supplied EPUB without reproducing its text. All 84 HTML documents parsed as XML. They contain images and links, but no h1/h2/h3, em/i, strong/b, list elements, or explicit EPUB semantic roles. This inspection does not establish rendering fidelity or identify note links. The rendering prototype must investigate styling and link conventions rather than assuming semantic tags are sufficient.

## Answer

Use consistent, reader-controlled typography like Omawrite while preserving headings, italics, bold, lists, images, and notes. Pause playback at images and chapter boundaries. Offer optional, skippable chapter feedback with a persistent disable setting; its remaining interaction and storage decisions belong to [chapter-end feedback](10-chapter-feedback.md).

Do not follow footnote markers by default. With following enabled, play the note and automatically resume the main text after its marker. Include a chapter list and Open File action; drag-and-drop is optional. Remember positions per book. On launch, reopen the last book at its saved position in conventional view with playback stopped.

Show clear placeholders for unsupported content and pause playback there. Explain opening failures and retain the current book. These user-confirmed behaviors settle the content contract; rendering, note detection, and position recovery still require technical proof in a dedicated prototype.

2026-10-20, user refinement: Distinguish title pages from chapters. Conventional reading must allow Left/Right Arrow page browsing through the book. The rendering prototype implements reflowed screen pages, including navigation across section boundaries.
