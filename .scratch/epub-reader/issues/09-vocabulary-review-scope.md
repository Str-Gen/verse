# Which definition lookup and recognition practice belong in the first release?

Id: 09
Parent: ../map.md
Labels: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Blocked by: 08

## Question

Given actual captured words and user feedback, decide the first-release scope of definition lookup and word-shape recognition practice. Define what the reader reviews, what useful progress means, and whether a trainer is part of the first release or a later effort. Jev ranking is not a source of generated dictionary definitions. Identify any research into dictionary sources or training evidence needed before this decision can close.

## Comments

2026-10-21: Resuming the map after acceptance of playback refinements. Capture/review policy is settled: five seconds or three sentences, unfamiliar versus known-but-slow feedback, dismissal, book-specific storage, and optional review on closing a book. This ticket determines what happens when the reader chooses to revisit those saved words. Definition sources and any training efficacy claims need separate evidence; user preferences determine the scope first.

2026-10-21, user choices: Show captured context and the dictionary definition directly in a two-column review. If a word has several meanings, show all available definition senses rather than selecting one. The user prefers a free local dictionary without an external lookup API. They accepted prototyping a short recognition session: display the word at the reading pace, reveal its captured sentence, and record comfortable/difficult recognition. This is an interaction experiment, not an established training benefit.

Initial source check: [Kaikki](https://kaikki.org/dictionary/) publishes structured Wiktionary data under Wiktionary's CC-BY-SA/GFDL licenses. Its [raw download page](https://kaikki.org/dictionary/rawdata.html) is the supported input path; older postprocessed downloads are deprecated. A local English-only index could retain all senses while excluding unrelated media and language data. Final package size, inflection lookup, attribution packaging, and coverage remain to be verified. [Princeton WordNet](https://wordnet.princeton.edu/license-and-commercial-use) permits local use and distribution without royalties with its notices retained; its brief glosses are an alternative, but are less aligned with the requested full dictionary entry. Prefer evaluating Wiktionary data for this UI, without committing the source until the prototype checks it.

The implementation experiment is [offline dictionary review and recognition practice](12-offline-review-and-practice.md). The subsequent user choice below settles scope; that prototype remains available for interaction review.

## Answer

The user selected a local SQLite dictionary built from Wiktionary data via Kaikki. Review places the captured context on the left and all available definition senses on the right. Missing entries explicitly say the local database does not have the word; there is no automatic remote fallback. Dictionary attribution and licensing travel with the data.

Practice belongs as a simple voluntary mode in the initial prototype. Show a saved word at the reader's selected pace, reveal context, and offer Comfortable, Difficult or Skip. No further scope interview is needed now. Session impressions are not comprehension measurements; adaptive scheduling and proven learning outcomes remain outside this draft. Implementation and user review evidence belong in ticket 12.
