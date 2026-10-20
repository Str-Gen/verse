# How should recent-context capture and Jev ranking build a useful personal word list?

Id: 08
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: claimed
Assignee: codex
Blocked by: 04

## Question

Prototype a shortcut that captures recently displayed text without requiring word-perfect reaction time. Compare the last three seconds with the last three encountered sentences. Use Jev to rank words that might have disrupted recognition for this reader, retain the captured context for correction, and build a reusable personal word list. Define a small interface that hides selection, ranking, persistence, and failure handling in a self-contained module. Keep definitions and training separate. Resolve through user feedback and measured model behavior, not assumptions about personal difficulty.

## Notes

Consult typesafe-ai, codebase-design, and prototype. Read current TypeSafe API, Score, state, confidence, and reranking guidance before integration. No production GUI choice is implied by a local experiment. Never put API keys in the browser or committed files.

## Prototype context

Branch: `prototype/vocabulary-capture`

Latest commit: `b16d4666cd6789ec2c2d4b22903f5b28ffcb2d9a`

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
