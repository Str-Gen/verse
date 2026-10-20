# What controls and pacing make the two reading modes feel right?

Id: 04
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: claimed
Assignee: codex
Blocked by: 01

## Question

With the user, explore conventional-view entry, shortcut activation, word or phrase presentation, pause/stop, backtracking, speed adjustment, and return location. Define what current passage and finished position mean in concrete examples. Use a disposable prototype informed by evidence; resolve only through user feedback.

## Research inputs

- [Speedreading evidence](../research/speedreading-evidence.md) supports testing user control, rereading, and boundary pauses; it does not establish optimal pacing defaults.
- [EPUB locations](../research/epub-and-location-options.md) identifies exact passage round-tripping as a technical proof obligation. The interaction prototype must distinguish the displayed word from the next word.

## Prototype context

Branch: `prototype/reading-interaction`

Latest commit: `df3a5ee8295f567a7bcc1afcaa291fc6a428ecee`

Original comparison commit: `acab9aa3fbc0a28374f860ed7ddd76f3297eb388`

Worktree: `/home/strgenix/speedreading-prototypes/reading-interaction`

Assets:

- [Prototype guide](/home/strgenix/speedreading-prototypes/reading-interaction/prototypes/reading-interaction/README.md)
- [Shareable demo with original sample text](/home/strgenix/speedreading-prototypes/reading-interaction/prototypes/reading-interaction/reading-interaction.prototype.html)
- [Private demo using the default EPUB](/home/strgenix/speedreading/omawrite/test-data/local/reading-interaction.html). This generated file contains book text and is ignored by Git.
- [Private demo with reference pacing](/home/strgenix/speedreading/omawrite/test-data/local/reading-interaction-v2.html).
- [Latest private demo with smaller compound pieces](/home/strgenix/speedreading/omawrite/test-data/local/reading-interaction-v3.html). Earlier versions remain available for comparison.

## Comments

2026-10-20: Built and opened a disposable local HTML prototype. It covers selecting a starting word, playback across paragraphs and sections, pause, sentence rewind, exact displayed-word return, end-of-excerpt, and pause on focus loss. Compare centered versus aligned words and optional boundary pauses. The initial pace and timing factors are provisional. This does not choose a production GUI toolkit.

The supplied book produced three narrative sections and 8,675 words under a deliberately limited extraction rule. Browser smoke checks exercised the transitions, visible highlight, timed advancement, and focus-loss pause without JavaScript exceptions. Conventional and aligned-word screenshots were visually inspected. This does not establish reading comfort or production EPUB source-location correctness.

Awaiting the user's reaction to word alignment and return behavior. Remaining discussion includes pace and boundary pauses, phrase versus single-word presentation, entry when no word is selected, and desired interruption/restart behavior. No interaction decisions are resolved yet; keep this ticket claimed until the live feedback settles its question.

2026-10-20, user feedback: the supplied recording's fixed bar helps keep focus aligned; 350 wpm feels like a nicer default; comma, semicolon, and sentence pacing feels more natural; long familiar words do not need extra time; slightly heavier text is preferred. These establish directions within this ticket, but do not settle the full interaction question.

Inspected `/home/strgenix/Videos/screenrecording-2026-10-20_20-19-41.mp4` and compared timed word changes against the EPUB. Approximate observed medians were 117 ms for ordinary words, 317 ms for commas and the single observed semicolon, 433 ms for matched sentence endings within paragraphs, and 617 ms for matched paragraph endings. The prototype guide records the method, counts, and limitations. Neither the private recording nor its extracted text was committed.

Revision 2 adds a stationary bracket-shaped focus guide, 350 wpm default, local Noto Serif at medium weight with alternatives, separate clause/sentence holds plus extra paragraph time, and em-dash piece splitting. Long words retain the ordinary hold. Pace now includes pauses over a section; this interpretation of the control remains a proposal. Animation-frame deadlines keep changes aligned to display refresh without a catch-up burst after interruptions.

The revised private demo opened in the user's browser. The browser check exercised transitions and observed ordinary holds of 116–134 ms without JavaScript exceptions. Actual comfort and preference still require the user's response. Exact return versus sentence return, centered versus aligned words, and single-word versus phrase display remain open. Keep this ticket claimed.

2026-10-20, further feedback: a large piece inside a parenthetical was too much to read at speed; an opening parenthesis can begin a sequence but must not group the entire aside into one presentation. Inspection found that parentheses already advanced word by word, but the first aside contained `eight-hundred-year-old` as a single unit. Treat this as the likely reported case, not confirmation of the user's exact stopping point.

Revision 3 splits letter-to-letter hyphenated compounds while preserving conventional text and keeping parentheses attached to adjacent words. The browser displayed `eight-`, `hundred-`, `year-`, `old` separately and returned to the last displayed piece. All fixture paragraphs reconstructed exactly from pieces and gaps. The revised file was opened for the user; the ticket remains claimed for continued interaction feedback.
