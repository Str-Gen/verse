# Which GUI and fork strategies fit Linux now and possible mobile ports later?

Id: 02
Parent: ../map.md
Labels: wayfinder:research
Type: research
Status: resolved
Assignee: codex-research-02
Blocked by: 

## Question

Inspect Omawrite reuse boundaries and compare credible Rust GUI strategies, including retaining Qt/QML with a Rust core. Evaluate Linux text rendering, input, accessibility, packaging, and future macOS, Windows, iOS, Android viability. Separate supported functionality from roadmap claims. Recommend a shortlist and concrete unknowns; do not select the architecture for the user.

## Research context

Branch: research/gui-and-fork-options
Worktree: /home/strgenix/speedreading-research/gui-and-fork-options
Findings: ../research/gui-and-fork-options.md

## Answer

Resolved 2026-10-20 by codex-research-02. [GUI and fork options](../research/gui-and-fork-options.md) records source inspection, current primary-source evidence, licenses, and remaining checks.

Shortlist Qt Quick/QML with a Rust core and Tauri with a Rust core; retain Slint as a candidate if normalized prose and a Rust-focused toolkit outweigh reuse. Omawrite uses a plain-text editor with Markdown highlighting, so it supplies useful shell behavior but no EPUB renderer. Parser choice is separate from text layout. Decide conventional-view fidelity and the meaning of native UI before selecting architecture. Qt supports mobile overall, but WebEngine is desktop-only; Slint now documents iOS Rust deployment, contrary to older indexed roadmap text. No GUI quality or mobile parity claim has been validated by a prototype.
