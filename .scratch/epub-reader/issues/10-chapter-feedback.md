# How should chapter-end feedback help the reader choose a comfortable pace?

Id: 10
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: claimed
Assignee: codex
Blocked by: 04

## Question

Prototype a small chapter-end prompt for feedback about playback speed and perceived understanding of words. Make skipping and continuing easy, and provide a persistent option to disable the prompt. Determine the questions, controls, stored context, and whether pace guidance belongs in the first release. Resolve through user feedback.

## Confirmed requirements

2026-10-20: The user requested a pause at chapter boundaries, optional feedback that is stored, and possible guidance toward a more pleasant or appropriate words-per-minute setting. Readers must be able to ignore the prompt and continue, or turn it off completely.

2026-10-20, user confirmation: Use "How did the pace feel?" with "too slow / comfortable / too fast", and "How well could you follow the text?" with "easily / mostly / struggled". Any suggested WPM change requires the reader's acceptance.

## Open decisions

- How the prompt, skip, and continue controls behave, including when prompts are disabled.
- What feedback and reading-session context to retain, and how users inspect or clear it.
- Whether to offer explicit pace suggestions in the first release or collect feedback first, and how to derive suggestions. Suggestions require explicit acceptance; do not change pace automatically.
- How to represent sessions where the pace changed during the chapter.

## Evidence boundary

Self-reported understanding is subjective feedback, not a measured comprehension score. Consult the existing [speedreading evidence](../research/speedreading-evidence.md) before proposing guidance. No scientifically optimal pace or improvement in comprehension has been established.

## Prototype context

Branch: `prototype/content-review`

Latest commit: `2145bdd3d41197fe10ca61798ee1764ef534af8c`

Original review commit: `480a79c4a31f7858415f920a79f0835651045c37`

Worktree: `/home/strgenix/speedreading-prototypes/content-review`

- [Live feedback study](http://127.0.0.1:8769/?study=feedback)
- [Run instructions, verification, and limitations](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/README.md)
- [Standalone synthetic sampler](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/sampler.prototype.html)

## Comments

2026-10-20: User authorized both feedback and rendering prototypes for review. Implemented agreed questions, skip/continue, persistent disable, saved feedback, export, and an optional 25 WPM suggestion requiring explicit acceptance. The step size and suggestion rule are experimental, not user-approved decisions. Preview responses are labelled separately. Feedback retains pace changes and source endpoints; no comprehension score is inferred.

Browser checks confirmed accepted suggestions and responses persist, skipping stores no answers, disabling survives reload, and the dialog fits desktop and narrow screens. Continue begins the next chapter; disabled prompts still leave a chapter-boundary pause. Keep this ticket claimed pending user review of the interaction, suggestion rule, and storage choices.

2026-10-20, user review correction: Title pages are not chapters and must not trigger chapter feedback. The revised prototype prompts only for identified chapters; front matter can advance without a check-in. The explicit feedback preview moves from front matter to the first identified chapter. Verified both title playback and preview behavior in Chromium.

2026-10-20: The user's request to avoid per-chapter review applies to vocabulary. Vocabulary review is on demand with a close-book reminder; this does not remove the separately agreed optional chapter pace/understanding check-in.
