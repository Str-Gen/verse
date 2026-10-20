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

2026-10-20: Built a disposable Python EPUB normalizer and browser study with reader-controlled typography, basic CSS-derived emphasis, chapter navigation, image/unsupported-content pauses, semantic footnote playback, local EPUB import, and per-book source-linked positions. No production toolkit or parser choice is implied. Imported books remain in ignored local storage; the committed sampler uses original synthetic text and imagery.

Verified all 189,147 extracted word pieces from the supplied EPUB against original XML document/node/offset/length locations. The extraction produced 84 spine sections, 85 image occurrences, 1,010 italic spans, and 14 bold spans. These counts describe this book only. Browser checks exercised default note skipping, automatic note return, interruptions, exact split-compound restoration, book switching, stopped restart, and an invalid upload retaining the active book without page JavaScript exceptions. Changed fingerprints produce an explicit chapter-start fallback.

Still requires user review and a broader EPUB corpus. Known limits include a restricted CSS subset, spine-item chapter boundaries, semantic-only note identification, inline-markup word fragmentation, and all images pausing even when decorative. Source locations are revision-specific XML paths, not EPUB CFI. Arbitrary SVG, tables, and media are not rendered; fixed-layout books are rejected. Keep the ticket claimed; this evidence does not close the production rendering/location proof obligation.

2026-10-20, user review: A title page is not a chapter. Conventional reading should support Left/Right Arrow to flip through pages. Revised the prototype to classify front matter separately, remove spine-index chapter numbering from the contents selector, and paginate conventional text into screen-sized pages. Buttons and arrow keys traverse pages and section boundaries; form fields retain arrow behavior. Page turns save their first source piece, so reopening restores the page with playback stopped. Font/window changes repaginate around the selected source.

Browser verification confirmed title-page traversal without feedback, forward/back navigation within long chapters and across sections, saved-page restoration, and preserved source location after resize, without JavaScript exceptions. Chapter classification still uses limited semantics/navigation heuristics and needs broader EPUB validation.

2026-10-20, user acceptance: "all works well, let's move ahead with more" after reviewing the page-browsing revision. Treat the revised front-matter distinction, conventional pagination, arrow-key browsing, and saved-page behavior as accepted interaction baselines. Remaining importer/corpus limitations are technical follow-up, not reasons to repeat the same interaction questions.
