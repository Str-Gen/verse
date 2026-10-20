# Can EPUB rendering preserve content and exact reading positions across both modes?

Id: 11
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: claimed
Assignee: codex
Blocked by: 03, 04, 05

## Question

Prototype the agreed content contract with source-linked positions before selecting the production renderer. Demonstrate conventional reading, playback, and exact passage return using the private supplied EPUB and shareable synthetic fixtures. Have the user assess typography, image interruptions, and footnote playback.

## Required evidence

- Preserve headings, emphasis, lists, images, and notes under reader-controlled typography. Inspect CSS-driven formatting as well as semantic markup. The supplied EPUB has no h1/h2/h3 or em/i/strong/b elements and no explicit EPUB semantic roles.
- Pause at images and unsupported content; distinguish reading content from decorative assets using a policy the user can review.
- Skip footnote markers by default; when enabled, play the note then return after the marker. Demonstrate note identification and safe behavior for ambiguous or broken links.
- Preserve source positions through split compounds, inline formatting, chapter navigation, book switching, and restart. Verify saved positions against the source, not only generated token indexes.
- Demonstrate clear feedback for unsupported content and failed opens without losing the current book or position.
- Document position-recovery behavior when the source book changes or a saved location cannot be restored, and bring any product tradeoff back to the user.

## Notes

Consult prototype. Keep user book text in ignored local assets. This experiment informs the architecture decision; it does not select a toolkit in advance or implement the production app. Chapter-feedback UI is tracked separately.

## Prototype context

Branch: `prototype/content-review`

Latest commit: `72ebbcfc7981a67928785db49fa7324a4876e66b`

Original review commit: `480a79c4a31f7858415f920a79f0835651045c37`

Worktree: `/home/strgenix/speedreading-prototypes/content-review`

- [Live content study](http://127.0.0.1:8769/?study=content)
- [Run instructions, verification, and limitations](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/README.md)
- [Standalone synthetic sampler](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/sampler.prototype.html)

## Comments

2026-10-21, user choice: Skip decorations during playback and pause on substantive images. Look ahead through the book to help identify repeated or mirrored ornaments. Repetition or mirroring is evidence for classification, not proof that an image is decorative; the classifier and uncertain-image behavior still need validation.

2026-10-21, confirmed position recovery: Reopening an unchanged EPUB restores the exact saved source position. If the reader replaces it with an edited edition and the exact position cannot be restored, offer a confidently and uniquely matched passage for confirmation. Otherwise offer the saved chapter's start, provided that chapter can be identified in the new edition. Do not silently select among repeated text matches or assume an old numeric section index identifies the same chapter. If neither passage nor chapter can be identified, preserve the old saved location and explain that the reader must choose a new location. This final no-match behavior is an implementation safeguard; the user explicitly accepted the matching-passage/chapter-start recovery policy.

2026-10-21: Resumed rendering investigation before architecture selection at the user's request. Fresh in-memory synthetic EPUB probes against the current prototype confirmed:

- `un<em>believ</em>able today` produces four playback words rather than two. The content model must join lexical words across inline formatting while retaining all contributing source spans. Deliberate compound display pieces remain a separate playback choice.
- A paragraph styled with `display:none` is included in both normalized output and playback. The content contract needs one consistent visibility interpretation before producing either view; reader-controlled typography does not justify exposing publisher-hidden text.
- Two navigation entries pointing to different fragments in one XHTML resource produce one chapter. Logical chapter ranges must be distinct from spine resources and preserve navigation fragments.
- Unrelated books with the same package identifier receive the same application book ID despite different fingerprints. Local book identity must be independent of publisher identifiers, with revision identity tracked separately, so positions, speed and history cannot leak across books.

These are observed prototype failures, not completed fixes. No private book text was emitted and no user state was changed. Subsequent user choices settled decorative-image skipping and changed-book position recovery above; the architecture ticket records Qt/QML with Rust preferred for Linux and a Tauri variant. Technical fixes and renderer-specific evidence remain open.

2026-10-20: Built a disposable Python EPUB normalizer and browser study with reader-controlled typography, basic CSS-derived emphasis, chapter navigation, image/unsupported-content pauses, semantic footnote playback, local EPUB import, and per-book source-linked positions. No production toolkit or parser choice is implied. Imported books remain in ignored local storage; the committed sampler uses original synthetic text and imagery.

Verified all 189,147 extracted word pieces from the supplied EPUB against original XML document/node/offset/length locations. The extraction produced 84 spine sections, 85 image occurrences, 1,010 italic spans, and 14 bold spans. These counts describe this book only. Browser checks exercised default note skipping, automatic note return, interruptions, exact split-compound restoration, book switching, stopped restart, and an invalid upload retaining the active book without page JavaScript exceptions. Changed fingerprints produce an explicit chapter-start fallback.

Still requires user review and a broader EPUB corpus. Known limits include a restricted CSS subset, spine-item chapter boundaries, semantic-only note identification, inline-markup word fragmentation, and all images pausing even when decorative. Source locations are revision-specific XML paths, not EPUB CFI. Arbitrary SVG, tables, and media are not rendered; fixed-layout books are rejected. Keep the ticket claimed; this evidence does not close the production rendering/location proof obligation.

2026-10-20, user review: A title page is not a chapter. Conventional reading should support Left/Right Arrow to flip through pages. Revised the prototype to classify front matter separately, remove spine-index chapter numbering from the contents selector, and paginate conventional text into screen-sized pages. Buttons and arrow keys traverse pages and section boundaries; form fields retain arrow behavior. Page turns save their first source piece, so reopening restores the page with playback stopped. Font/window changes repaginate around the selected source.

Browser verification confirmed title-page traversal without feedback, forward/back navigation within long chapters and across sections, saved-page restoration, and preserved source location after resize, without JavaScript exceptions. Chapter classification still uses limited semantics/navigation heuristics and needs broader EPUB validation.

2026-10-20, user acceptance: "all works well, let's move ahead with more" after reviewing the page-browsing revision. Treat the revised front-matter distinction, conventional pagination, arrow-key browsing, and saved-page behavior as accepted interaction baselines. Remaining importer/corpus limitations are technical follow-up, not reasons to repeat the same interaction questions.
