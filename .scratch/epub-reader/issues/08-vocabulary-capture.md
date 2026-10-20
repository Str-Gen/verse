# How should recent-context capture and Jev ranking build a useful personal word list?

Id: 08
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: resolved
Assignee: codex
Blocked by: 04

## Question

Prototype a shortcut that captures recently displayed text without requiring word-perfect reaction time. Compare the last five seconds with the last three encountered sentences. Use Jev to rank words that might have disrupted recognition for this reader, retain the captured context for correction, and build a reusable personal word list. Define a small interface that hides selection, ranking, persistence, and failure handling in a self-contained module. Keep definitions and training separate. Resolve through user feedback and measured model behavior, not assumptions about personal difficulty.

## Notes

Consult typesafe-ai, codebase-design, and prototype. Read current TypeSafe API, Score, state, confidence, and reranking guidance before integration. No production GUI choice is implied by a local experiment. Never put API keys in the browser or committed files.

## Prototype context

Branch: `prototype/vocabulary-capture`

Latest commit: `f2ffeb1515acd2d3ec379da8e8bfd339c5e97ec9`

Original prototype commit: `b16d4666cd6789ec2c2d4b22903f5b28ffcb2d9a`

Worktree: `/home/strgenix/speedreading-prototypes/vocabulary-capture`

- [Module guide and verification evidence](/home/strgenix/speedreading-prototypes/vocabulary-capture/prototypes/vocabulary-capture/README.md)
- [Deep module](/home/strgenix/speedreading-prototypes/vocabulary-capture/prototypes/vocabulary-capture/vocabulary.py)
- [Local experiment](http://127.0.0.1:8768/), while its backend is running.

## Comments

2026-10-20: The user confirmed that capture should keep playback running, with review later. The user supplied a local key file at `/home/strgenix/speedreading/jev-speedreading.key`. The backend reads it directly; it is outside the repository, has owner-only permissions, and was not displayed or committed.

Built an isolated Python module and thin local-browser adapter without changing the existing reader files. The implementation is an experiment in the module's interface, not a native language or GUI decision. The module owns time/sentence window selection, complete lexical candidate reconstruction, persistence, model prompts, typed-response validation, background jobs, and deduplicated saved words.

C captures recent playback using a selectable three-second or three-sentence window. V pauses and opens review. Only the already-displayed part of the current sentence is captured. Split compounds become one vocabulary candidate only after all pieces were displayed. Keep is an explicit reader choice; the model does not silently populate the personal word list. Existing captures survive unavailable credentials, request failures, and restarts.

Read live TypeSafe API, Score, state, confidence, model, and reranking documentation. Use one comparable recognition-practice Score per candidate in a shared state request, pinned to `jev-1.13.0`. Scores are graded judgments, not calibrated personal probabilities of having caused the interruption. Raw distributions and usage are retained.

Offline module checks verified time selection excluding pauses, unread-tail exclusion, partial-compound handling, deduplication, persistence, and reader choices during an in-flight request. Two small live requests on invented text verified the contract and browser flow: an uncommon-word example ranked the intended rare words above ordinary words; the UI capture returned all 13 requested judgments, kept playback running, and persisted a kept word across reload. No JavaScript exceptions occurred. These are integration observations, not evidence of personalized accuracy.

The user's backend runs on localhost with an initially empty private database at `test-data/local/vocabulary.sqlite`; separate temporary databases held all test captures. The live experiment was opened in the browser.

Still open: user preference for capture window; whether the actual distracting word ranks near the top in real reading; review presentation; the depth of per-reader feedback; and whether this module interface remains sufficient for the eventual native core. Definitions and training are covered by the separate review-scope ticket. Keep this ticket claimed until the user has tried the feature.

2026-10-20, first real reading feedback: both user captures completed, each with 16 candidates and 16 Jev judgments. The top-ranked words were `retching` and `whorls`; the user confirmed these were the intended words and said the feature works well. They know what retching means, while whorls is a word they never use. This supports the recognition-familiarity framing rather than assuming unknown meanings. Two successful captures are promising examples, not a measured general accuracy rate.

The user also requested an explicit progress counter. Reader version 4 displays `Chapter N · X / Y words` and drives the progress bar from the same chapter word count. Split hyphenated compounds count once. The generated fixture has chapter totals of 1,101, 5,286, and 2,330 words. Verified chapter transitions reset the current count, the four pieces of `eight-hundred-year-old` all use word index 374, and the served page renders the new label while retaining capture controls. This reader-only change leaves the Jev module unchanged.

2026-10-20: Resuming the outstanding vocabulary-capture decision after the user accepted conventional page browsing. The existing adapter defaults to the last three seconds and offers the last three sentences. Settle the preferred default and the depth of reader feedback before moving to definition/practice scope. Preserve the already confirmed non-interrupting capture and successful ranking examples.

2026-10-20, user decisions: Capture the last five seconds or three sentences. Review offers unfamiliar word, known but slow to recognize, and dismissal. Vocabulary review should not occur every chapter; suggest it gently when closing a book by reminding the reader that the book's word list is stored for later review. This concerns vocabulary review, not the separate chapter pace/understanding check-in.

Implemented the five-second default, retained the three-sentence alternative, stored per-candidate feedback, filtered review by book, and added an optional review link on the single-book prototype's closed-book screen. Existing captures keep their historical window labels and saved words. The module interface now includes `review(document)` and `feedback(capture, candidate, choice)`. No model API or ranking-policy change was needed.

Verified with temporary storage and no model calls: five active seconds across pauses, both feedback categories, dismissal and correction, book isolation, retention of other evidence for the same word, restart persistence, and close/review/reopen in Chromium without page exceptions. The single-book prototype demonstrates closing; combining this behavior with the separate content prototype's book switcher remains integration work.

## Answer

Adopt non-interrupting capture of the last five seconds of displayed reading, with the last three encountered sentences as an alternative. Preserve captured context, exclude unread sentence tails and incomplete words, and use Jev ranking as suggestions that the reader can correct. The two real-reading examples support the initial workflow, not a general accuracy claim.

Save reader choices as unfamiliar or known-but-slow-to-recognize; support dismissal and later correction. Keep captures and chosen words associated with their book. Review is on demand, with an optional reminder on closing a book that the word list is stored for later. Do not request vocabulary review after each chapter. Keep window selection, ranking, persistence, feedback, and failures behind the existing self-contained module interface.

Definition lookup and recognition training remain separate scope decisions. The updated UI is available for further review; these explicit user choices settle the initial capture/review policy.
