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

Commit: `acab9aa3fbc0a28374f860ed7ddd76f3297eb388`

Worktree: `/home/strgenix/speedreading-prototypes/reading-interaction`

Assets:

- [Prototype guide](/home/strgenix/speedreading-prototypes/reading-interaction/prototypes/reading-interaction/README.md)
- [Shareable demo with original sample text](/home/strgenix/speedreading-prototypes/reading-interaction/prototypes/reading-interaction/reading-interaction.prototype.html)
- [Private demo using the default EPUB](/home/strgenix/speedreading/omawrite/test-data/local/reading-interaction.html). This generated file contains book text and is ignored by Git.

## Comments

2026-10-20: Built and opened a disposable local HTML prototype. It covers selecting a starting word, playback across paragraphs and sections, pause, sentence rewind, exact displayed-word return, end-of-excerpt, and pause on focus loss. Compare centered versus aligned words and optional boundary pauses. The initial pace and timing factors are provisional. This does not choose a production GUI toolkit.

The supplied book produced three narrative sections and 8,675 words under a deliberately limited extraction rule. Browser smoke checks exercised the transitions, visible highlight, timed advancement, and focus-loss pause without JavaScript exceptions. Conventional and aligned-word screenshots were visually inspected. This does not establish reading comfort or production EPUB source-location correctness.

Awaiting the user's reaction to word alignment and return behavior. Remaining discussion includes pace and boundary pauses, phrase versus single-word presentation, entry when no word is selected, and desired interruption/restart behavior. No interaction decisions are resolved yet; keep this ticket claimed until the live feedback settles its question.
