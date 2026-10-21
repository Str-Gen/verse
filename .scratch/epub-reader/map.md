# Find the way to a simple EPUB reader

Labels: wayfinder:map
Status: open

## Destination

A build-ready specification for a Linux-first, native EPUB reader with conventional reading and smooth, focused speedreading. Resolve the product, content, architecture, and validation decisions needed to begin implementation.

## Notes

- This is a planning effort. Research and disposable prototypes support decisions; implementation follows this map.
- Consult wayfinder, grilling, and domain-modeling for decision sessions; research for research tickets; prototype for interaction experiments.
- User-confirmed requirements: local DRM-free EPUBs; remember reading position; fiction and nonfiction, primarily text; comfortable focus and good comprehension take priority over maximum speed.
- Enter speedreading at the current passage with one shortcut; continue through paragraphs until stopped; return to conventional reading where playback finished.
- Linux with Omarchy integration is the primary target for personal use. The user now requests both Qt/QML and Tauri variants, with Tauri providing the route to other platforms. Platform release commitments remain undecided; toolkit support alone does not establish working ports.
- Omawrite is the intended Linux fork and visual reference. Prefer Rust application logic behind Qt/QML, retaining C++ as a fallback if integration is not workable. The architecture ticket records this direction and the proposed shared core. The user has not programmed a Linux GUI before.
- Default local test book: `test-data/local/default.epub`, supplied by the user and moved from Downloads. This directory is ignored by Git. Do not commit or redistribute its contents. The rendering prototype has exercised this book and checked extracted pieces against source locations; broader EPUB compatibility remains unverified.
- New user-requested scope: optional Jev-assisted capture of recently displayed words for a personal review list. Keep the integration in a self-contained deep module. Dictionary definitions and recognition practice require a separate scope decision.
- Product name is deferred.
- Tracker: local Markdown, following /home/strgenix/.agents/skills/setup-matt-pocock-skills/issue-tracker-local.md. Tickets have stable numbered identities, Type, Labels, Parent, Status, Assignee, and Blocked by fields. Open unassigned tickets whose blockers are resolved form the frontier, ordered by number. Claim before work; append resolutions under Answer. The map indexes resolutions only.
- Planning lives on planning/epub-reader in the local Omawrite checkout. This is not yet a hosted fork.

## Decisions so far

- [Which speedreading behaviors support comfortable reading and comprehension?](issues/01-speedreading-evidence.md): Preserve reader control and rereading; timing and focus benefits need prototype validation, and higher speed does not guarantee retained comprehension.
- [Which EPUB libraries and location models can support both reading modes?](issues/03-epub-and-location-options.md): Evaluate rbook first; parsing and layout are separate, and source-linked positions need a round-trip proof before renderer selection.

- [Which GUI and fork strategies fit Linux now and possible mobile ports later?](issues/02-gui-and-fork-options.md): Shortlist Qt/QML with Rust and Tauri with Rust; compare preserved book formatting with normalized prose before choosing, with Slint as a third candidate.

- [What controls and pacing make the two reading modes feel right?](issues/04-reading-interaction.md): Revision 3 is accepted as the starting interaction; retain focus guide, 350 wpm, natural boundary pacing, and smaller compound pieces while allowing later refinement.

- [What EPUB content and reading-position behavior does the first release promise?](issues/05-content-contract.md): Reader-controlled typography with retained structure, image pauses, optional footnote playback, chapter feedback, per-book resume, and explicit failure feedback; drag-and-drop is optional.

- [How should recent-context capture and Jev ranking build a useful personal word list?](issues/08-vocabulary-capture.md): Five seconds or three sentences, explicit unfamiliar/recognition feedback or dismissal, book-specific review on demand, and an optional close-book reminder.

- [Which definition lookup and recognition practice belong in the first release?](issues/09-vocabulary-review-scope.md): Local Kaikki/Wiktionary definitions with all senses beside captured context, explicit missing-entry feedback, and a simple optional practice draft.

- [How should chapter-end feedback help the reader choose a comfortable pace?](issues/10-chapter-feedback.md): Optional chapter feedback with explicit 25 WPM suggestions, saved speed per book, and local per-book history that can be viewed and cleared.

## Not yet specified

- Later refinements to the accepted pacing and interaction after longer reading sessions.
- Packaging and verification details after architecture and first-release behavior are settled.

## Out of scope

- Writing and editing books.
- Guaranteed simultaneous releases on every platform before platform-specific validation.
- A browsable library, accounts, cloud sync, and book acquisition for the first release.
- DRM removal, non-EPUB formats, and graph/table-heavy books for the first release.
- Product naming and branding.
- Production implementation during this planning map.

## Overnight integration evidence, 2026-10-21

All four overnight branches were merged with ancestry retained on `overnight/20261021-integration`. The runnable layout is `prototypes/overnight/{core,qt,tauri,validation}` with integration launch/check scripts in `prototypes/overnight/integration`. Both frontends import through the same Rust CLI/library. CXX-Qt 0.10 works on this Linux host, so no handwritten C++ application-logic fallback was necessary. Qt/QML with Rust remains the preferred Linux/Omarchy direction; Tauri remains a second renderer over the content core.

Native Qt and Tauri tests now consume regenerated Rust exports and independent synthetic Unicode/hidden-text fixtures. The independent oracle exposed and verified a fix for words containing astral symbols. Live Qt theme-file watching passes synthetic replacement and missing-directory tests. Source/restart/speed and changed-edition confirmation checks pass within the tested cases. See the root `MORNING-REPORT.md` and `prototypes/overnight/integration/README.md` for commands, exact outcomes, native evidence and limitations.

Tickets 06, 07 and 11 remain open. Shared playback/recovery/persistence logic is not fully consolidated; the generic independent native action-trace drivers are pending; Qt optional note playback and feedback remain incomplete. SVG rendering fails the independent suite's image expectation with explicit unsupported placeholders; its PNG equivalent passes. This session did not test mobile ports or obtain user acceptance of visual quality. Accepted decoration skipping and changed-book recovery defaults remain unchanged.
