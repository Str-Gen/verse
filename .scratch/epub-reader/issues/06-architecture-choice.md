# Which architecture should the reader implement?

Id: 06
Parent: ../map.md
Labels: wayfinder:grilling
Type: grilling
Status: open
Assignee: 
Blocked by: 02, 03, 04, 05

## Question

Choose the GUI, language boundaries, EPUB processing and rendering approach, and position model using research and the agreed interaction/content contract. Specify what is retained from Omawrite and what is replaced, including future platform tradeoffs and any prototype needed before commitment.

## Research inputs

- [GUI and fork options](../research/gui-and-fork-options.md) shortlists Qt/QML with Rust and Tauri with Rust, with Slint for normalized prose. Resolve whether an installed webview application meets the intended native experience.
- [EPUB and location options](../research/epub-and-location-options.md) recommends evaluating rbook first, with source-linked token spans. These are candidates, not adopted dependencies.
- A chapter-rendering and location-mapping experiment may be needed before this decision can close. Create a dependent prototype ticket if the earlier interaction/content work does not establish the needed evidence.
