# How should offline definitions and a short recognition session support saved-word review?

Id: 12
Parent: ../map.md
Labels: wayfinder:prototype
Type: prototype
Status: claimed
Assignee: codex
Blocked by: 08

## Question

Prototype the user-approved two-column review: captured sentence on the left and a local dictionary entry with all available senses on the right. Evaluate Wiktionary-derived local data first, with source attribution and version information. Prove lookup works without network access. Handle inflected forms such as retching and whorls, multiple meanings, and missing entries explicitly. Measure the proposed dictionary package size before promising a download footprint.

Prototype a short, voluntary recognition session for saved words: show the word at the selected reading pace, reveal its captured sentence, and offer comfortable/difficult feedback. Keep this feedback distinct from measured comprehension or proven learning. Review remains on demand, with the previously agreed close-book reminder.

## Notes

Consult prototype. Preserve existing captures and book-specific review choices. Do not send book text to a dictionary service. Prefer local bundled or separately installed dictionary data; no paid or runtime external lookup API. Retain all definition senses from the selected source rather than generating or guessing a single meaning. Confirm source licensing and attribution against the actual dataset used. Capture-window context can be partial; label it honestly rather than inventing missing sentence text.

This experiment supplies evidence to the definition/practice scope decision. No production GUI or storage choice is implied.

User approved Kaikki-derived local SQLite, an explicit missing-entry message in the right column, and a simple practice draft without further scope questions.

## Prototype evidence

Implemented in `/home/strgenix/speedreading-prototypes/vocabulary-capture`, branch `prototype/vocabulary-capture`, implementation commit `f87b681`. Live review: http://127.0.0.1:8768/ . Open Word list, select a saved word or captured candidate, or choose Practice saved words. The prototype remains claimed for the user's interaction review.

The complete supported Kaikki raw dump dated 2026-09-02 is installed locally. The English SQLite database contains 1,492,836 entry records and occupies 416,641,024 bytes. Records include separate etymologies and parts of speech. All supplied definition glosses and labels remain available; direct base-entry references resolve inflections, while deeper reference chains remain as definition text. Exact spelling wins over case-insensitive fallback. Source metadata, archive checksum, attribution notice and CC BY-SA 4.0 license text accompany the importer. Generated data and book text remain outside version control.

Review uses captured context on the left and definitions on the right, stacking on narrow screens. Partial captured context is labelled. Missing words display the requested local-database message, distinct from an absent installation. No definition API or generated definition fallback exists.

Practice shows each saved word for one word interval at the chosen WPM, reveals captured context and definitions on request, and offers Comfortable, Difficult and Skip. Session counts are temporary in this draft; existing word classifications remain unchanged. No adaptive scheduler or comprehension claim is included.

Manual import checks covered multiple senses, case handling, lemma lookup, missing/uninstalled states and failed-import preservation. Chromium verified real `retching` and `whorls` entries, missing-word feedback, responsive layout, and practice reveal/feedback/skip/exit. Dictionary browsing made no external requests while external network routes were blocked, and no JavaScript exceptions occurred. Tests used separate capture storage with no Jev requests.
