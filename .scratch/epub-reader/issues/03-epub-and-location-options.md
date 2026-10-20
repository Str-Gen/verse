# Which EPUB libraries and location models can support both reading modes?

Id: 03
Parent: ../map.md
Labels: wayfinder:research
Type: research
Status: resolved
Assignee: codex-research-03
Blocked by: 

## Question

Investigate maintained Rust EPUB crates and primary EPUB specifications. Separate archive/package parsing from XHTML/CSS rendering. Assess text extraction in reading order, navigation, stable position mapping between conventional view and playback, EPUB version coverage, licensing, and limitations. Recommend candidates and validation questions for local DRM-free text-focused books.

## Research context

Branch: research/epub-and-location-options
Worktree: /home/strgenix/speedreading-research/epub-and-location-options
Findings: ../research/epub-and-location-options.md

## Answer

Resolved on 2026-10-20. Rust EPUB parsing is feasible. Evaluate `rbook` first, with `epub` as a comparison; neither removes the need for XHTML layout and shared source-location mapping. Preserve source spans during tokenization and prove exact passage round-tripping before selecting the renderer. CFI and Readium-style locators offer reference models, with explicit Unicode offset handling required.

See [EPUB libraries and reading locations](../research/epub-and-location-options.md) for dated package evidence, limitations, location recommendations and validation gates. No user EPUB or rendering prototype was tested. This resolves the investigation, not the architecture or content-policy decisions.
