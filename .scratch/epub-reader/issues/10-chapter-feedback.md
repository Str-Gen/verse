# How should chapter-end feedback help the reader choose a comfortable pace?

Id: 10
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: resolved
Assignee: codex
Blocked by: 04

## Question

Prototype a small chapter-end prompt for feedback about playback speed and perceived understanding of words. Make skipping and continuing easy, and provide a persistent option to disable the prompt. Determine the questions, controls, stored context, and whether pace guidance belongs in the first release. Resolve through user feedback.

## Confirmed requirements

2026-10-20: The user requested a pause at chapter boundaries, optional feedback that is stored, and possible guidance toward a more pleasant or appropriate words-per-minute setting. Readers must be able to ignore the prompt and continue, or turn it off completely.

2026-10-20, user confirmation: Use "How did the pace feel?" with "too slow / comfortable / too fast", and "How well could you follow the text?" with "easily / mostly / struggled". Any suggested WPM change requires the reader's acceptance.

## Decision status

The remaining interaction, pace guidance, and history choices were accepted on 2026-10-21. See Answer for the agreed behavior.

## Evidence boundary

Self-reported understanding is subjective feedback, not a measured comprehension score. Consult the existing [speedreading evidence](../research/speedreading-evidence.md) before proposing guidance. No scientifically optimal pace or improvement in comprehension has been established.

## Prototype context

Branch: `prototype/content-review`

Latest commit: `72ebbcfc7981a67928785db49fa7324a4876e66b`

Original review commit: `480a79c4a31f7858415f920a79f0835651045c37`

Worktree: `/home/strgenix/speedreading-prototypes/content-review`

- [Live feedback study](http://127.0.0.1:8769/?study=feedback)
- [Run instructions, verification, and limitations](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/README.md)
- [Standalone synthetic sampler](/home/strgenix/speedreading-prototypes/content-review/prototypes/content-review/sampler.prototype.html)

## Comments

2026-10-21: Resuming this claimed decision in the next wayfinder session. Settle chapter-boundary continuation, whether simple explicit pace suggestions ship initially, and feedback retention/inspection. Existing questions and optional disable behavior remain confirmed; recommendations in the conversation are pending user choices.

2026-10-20: User authorized both feedback and rendering prototypes for review. Implemented agreed questions, skip/continue, persistent disable, saved feedback, export, and an optional 25 WPM suggestion requiring explicit acceptance. The step size and suggestion rule are experimental, not user-approved decisions. Preview responses are labelled separately. Feedback retains pace changes and source endpoints; no comprehension score is inferred.

Browser checks confirmed accepted suggestions and responses persist, skipping stores no answers, disabling survives reload, and the dialog fits desktop and narrow screens. Continue begins the next chapter; disabled prompts still leave a chapter-boundary pause. Keep this ticket claimed pending user review of the interaction, suggestion rule, and storage choices.

2026-10-20, user review correction: Title pages are not chapters and must not trigger chapter feedback. The revised prototype prompts only for identified chapters; front matter can advance without a check-in. The explicit feedback preview moves from front matter to the first identified chapter. Verified both title playback and preview behavior in Chromium.

2026-10-20: The user's request to avoid per-chapter review applies to vocabulary. Vocabulary review is on demand with a close-book reminder; this does not remove the separately agreed optional chapter pace/understanding check-in.

## Answer

2026-10-21: The user accepted the proposed defaults and confirmed that speed may be saved per book.

- Pause at chapter boundaries. Offer Save and continue and Skip and continue; both start the next chapter. Skipping stores no feedback answers. When feedback is disabled, retain the pause with a single Continue action. Title pages and other front matter do not trigger chapter feedback.
- Include explicit pace suggestions in the first release. Too fast offers a decrease of 25 WPM; too slow offers an increase of 25 WPM; comfortable offers no change. Apply changes only when the reader accepts them. Record understanding answers separately; they do not automatically determine pace changes.
- Remember the selected reading speed separately for each book, including accepted suggestions. Reopening a book restores its saved speed. The previously accepted 350 WPM baseline applies before a book has a saved speed.
- Keep local, per-book feedback history with the chapter, date, answers, and pace changes during that chapter. Provide a simple history view and a clear-history action for each book. Preserve the sequence of pace changes rather than representing a varying chapter pace as one fixed speed.

These are product decisions. The 25 WPM increment is an initial interaction choice, and understanding answers remain subjective feedback. The existing prototype supplies interaction evidence; this resolution does not claim that every newly settled persistence or history requirement has already been implemented.
