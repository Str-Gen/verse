# Which architecture should the reader implement?

Id: 06
Parent: ../map.md
Labels: wayfinder:grilling
Type: grilling
Status: open
Assignee: 
Blocked by: 02, 03, 04, 05, 08, 11

## Question

Choose the GUI, language boundaries, EPUB processing and rendering approach, and position model using research and the agreed interaction/content contract. Specify what is retained from Omawrite and what is replaced, including future platform tradeoffs and any prototype needed before commitment.

## Research inputs

- [GUI and fork options](../research/gui-and-fork-options.md) shortlists Qt/QML with Rust and Tauri with Rust, with Slint for normalized prose. Resolve whether an installed webview application meets the intended native experience.
- [EPUB and location options](../research/epub-and-location-options.md) recommends evaluating rbook first, with source-linked token spans. These are candidates, not adopted dependencies.
- A chapter-rendering and location-mapping experiment may be needed before this decision can close. Create a dependent prototype ticket if the earlier interaction/content work does not establish the needed evidence.

## User direction

2026-10-21: Prefer Qt/QML for Linux with Omawrite's quiet appearance and live Omarchy styling integration. Prefer implementing application logic in Rust while retaining QML; C++ is an acceptable fallback if Rust integration is not workable. Small footprint is an objective to measure, not an established Qt-versus-Tauri result.

The user also requests a Tauri variant for other platforms, potentially in another fork. Proposed structure: one reusable Rust content, location, playback-policy, and persistence core with Qt/QML and Tauri adapters. Repository layout is not settled. Investigate CXX-Qt as the Rust/QML bridge; generated C++ and Qt runtime dependencies remain even when application logic is Rust. See [CXX-Qt documentation](https://kdab.github.io/cxx-qt/book/).

Tauri supplies a cross-platform route, not immediate verified ports. Platform SDKs, packaging, file access, lifecycle behavior and device validation remain required. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Specific first-release platform commitments remain to be settled under readiness. Rendering and Rust/QML integration evidence must precede closing this architecture ticket.
