# Browser verification

Measured on 2026-10-21 using Playwright 1.64.1 and `/usr/bin/chromium`, browser version 154.0.8037.97, headless Linux, 1100 × 800 viewport. Each run created fresh in-memory browser contexts with isolated local storage. Only bundled synthetic fixtures were used, including the actual Rust-core demo export exercised by the built-in suite. The HTTP server bound to 127.0.0.1:18772.

Commands, from this component's directory, in separate terminals:

```sh
./browser.sh
```

```sh
PLAYWRIGHT_MODULE=/home/strgenix/.local/share/mise/installs/npm-playwright/1.64.1/node_modules/.mise/playwright@1.64.1/node_modules/playwright/index.mjs node tests/browser-check.mjs
```

`PLAYWRIGHT_MODULE` accepts another installed Playwright module's absolute `index.mjs` path. No test dependencies were downloaded or manifests added. The script prints its JSON results and exits nonzero when a check fails.

The final run exited 0. Six UI behavior checks and the built-in DOM suite passed. After the core-export integration, the built-in suite reported 39 checks, all passing, and no errors. This includes core JSON import, local embedded raster resources, three-fragment source mapping, manual note opening, default note skipping, optional note playback and return, and unresolved-note pausing. The UI run captured no uncaught page errors.

The six UI checks used actual Playwright mouse/keyboard/file-input operations for:

- ArrowRight and ArrowLeft pagination.
- Mouse drag across four rendered pieces, checking a nonempty browser selection and the selected starting source piece.
- Space starting playback from that piece, Space pausing after 400 ms, no advancement for another 250 ms, and Escape returning to the last displayed piece.
- Keyboard End on the font-size slider, changing it to 32 px while retaining the source piece through reflow.
- Entering 425 WPM, reloading, and verifying stopped reading mode with the same source piece and speed.
- Uploading an invalid schema-version JSON file and verifying the rejection notice with the existing book and source position retained.

The initial runner execution had a test-runner mistake: it asserted the built-in report's nonexistent `ok` property instead of `passed`. The UI checks and all 31 built-in checks passed in that run too. The property assertion was corrected and the full runner rerun successfully. No frontend code was changed by the browser verifier. After the implementation agent added core export and note/resource support, the entire browser runner was run again and exited 0 with all six UI checks and all 39 DOM checks passing.

The screenshot `.evidence/chromium-synthetic.png` contains only synthetic content and is locally ignored by Git. Visual inspection confirmed the large-font layout, selected word highlight, page counter, controls, and invalid-import notice were legible and contained within the viewport.

These results validate Chromium browser behavior only. They do not validate WebKitGTK/Tauri rendering, native IPC or Rust-backed persistence. The report's native evidence must come from separate native execution. This pass did not exercise real EPUB imports, accessibility tools, touch input, multiple monitors, or other operating systems. The built-in suite drives several application methods directly; only the six UI checks above claim external input coverage.

The implementation agent ran the full browser script again after the final keyboard and pending-feedback-state changes. It exited 0 with all six external-input checks and all 41 DOM checks passing. The added checks cover stopped source-matching startup and Escape after playback begins with button focus. Local output: `.evidence/browser-final.json`.
