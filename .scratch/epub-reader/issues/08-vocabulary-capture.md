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
