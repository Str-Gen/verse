# What makes the specification ready to build and verify?

Id: 07
Parent: ../map.md
Labels: wayfinder:grilling
Type: grilling
Status: open
Assignee: 
Blocked by: 04, 05, 06, 09, 10

## Question

Agree the acceptance scenarios, remaining technical proof obligations, Linux packaging target, and implementation sequence. Define the specification structure and ensure every requirement has a verification method. Resolve remaining decisions before declaring the map complete; this ticket does not implement the app.

## Overnight readiness update, 2026-10-21

Runnable Linux Qt/QML and Tauri prototypes now share the Rust content importer on `overnight/20261021-integration`. Reproducible launches and tests are documented in the integration-root `MORNING-REPORT.md`. This is prototype evidence, not production readiness.

Keep this ticket open for shared application-policy consolidation, unresolved rendering/interaction limitations, a broader publication corpus, independent UI-trace automation, packaging and port validation, and the user's visual review. Vocabulary/definition/practice work is not implemented by this integration. No Windows, macOS, Android or iOS test is claimed.
