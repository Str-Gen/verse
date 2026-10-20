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
- Linux is the first target for personal use. Technology choices must account for possible macOS, Windows, iOS, and Android support later; those ports are outside this effort.
- Omawrite is the intended fork and visual reference. Rust is a preference to investigate, not a settled requirement. The user has not programmed a Linux GUI before.
- Default local test book: `test-data/local/default.epub`, supplied by the user and moved from Downloads. This directory is ignored by Git. Do not commit or redistribute its contents. ZIP integrity and EPUB container presence were checked; rendering and compatibility have not been tested.
- Product name is deferred.
- Tracker: local Markdown, following /home/strgenix/.agents/skills/setup-matt-pocock-skills/issue-tracker-local.md. Tickets have stable numbered identities, Type, Labels, Parent, Status, Assignee, and Blocked by fields. Open unassigned tickets whose blockers are resolved form the frontier, ordered by number. Claim before work; append resolutions under Answer. The map indexes resolutions only.
- Planning lives on planning/epub-reader in the local Omawrite checkout. This is not yet a hosted fork.

## Decisions so far

- [Which speedreading behaviors support comfortable reading and comprehension?](issues/01-speedreading-evidence.md): Preserve reader control and rereading; timing and focus benefits need prototype validation, and higher speed does not guarantee retained comprehension.
- [Which EPUB libraries and location models can support both reading modes?](issues/03-epub-and-location-options.md): Evaluate rbook first; parsing and layout are separate, and source-linked positions need a round-trip proof before renderer selection.

- [Which GUI and fork strategies fit Linux now and possible mobile ports later?](issues/02-gui-and-fork-options.md): Shortlist Qt/QML with Rust and Tauri with Rust; compare preserved book formatting with normalized prose before choosing, with Slint as a third candidate.

## Not yet specified

- Details of pacing, interruptions, and interaction refinements after evidence and user reactions.
- Content edge cases and reading-position recovery exposed by the supplied EPUB and content-policy discussion.
- Packaging and verification details after architecture and first-release behavior are settled.

## Out of scope

- Writing and editing books.
- Shipping non-Linux ports in the first release.
- A browsable library, accounts, cloud sync, and book acquisition for the first release.
- DRM removal, non-EPUB formats, and graph/table-heavy books for the first release.
- Product naming and branding.
- Production implementation during this planning map.
