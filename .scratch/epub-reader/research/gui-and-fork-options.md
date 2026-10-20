# GUI and fork options for the EPUB reader

Research date: 2026-10-20. This resolves the investigation, not the architecture decision. Documentation under `latest`, `main`, and `qt-6` moves; select and verify release versions before implementation. No GUI prototype or device test was performed.

## Recommendation

Shortlist Qt Quick/QML with a Rust core and Tauri with a Rust core. Qt preserves the most useful Omawrite code and desktop behavior. Tauri supplies browser text layout, which is attractive if conventional reading should preserve EPUB HTML/CSS. Keep Slint as the third candidate if a Rust-focused stack and deliberately normalized book styling matter more than code reuse.

The decisive question is the conventional reader's fidelity. A native text widget displaying normalized paragraphs is a different commitment from a browser displaying book markup. Both can support excellent prose reading, but neither an EPUB parsing crate nor a GUI toolkit makes this decision disappear. Do not rewrite Omawrite into Rust before resolving it.

These are engineering judgments from the evidence below, not measured quality rankings.

## What the fork actually supplies

Inspected local Omawrite source on research branch rooted at commit `995206687ec325c6b129eb8d8d37077b9e6c4604`; upstream remote is `git@github.com:omacom/omawrite.git`.

- `src/Main.qml:633` uses `TextEdit.PlainText`. `src/backend.cpp:280` attaches `MarkdownHighlighter`. This is an editable Markdown source view, not a reusable HTML or EPUB renderer.
- Reusable pieces include the quiet window layout, footer, font picker, shortcuts dialog, zoom, theme integration, and Linux portal setup. `src/main.cpp` selects the desktop portal; `src/systemtheme.cpp` reads portal settings over D-Bus. `src/Main.qml:659` already treats fractional scaling differently when rendering text.
- Autosave, recovery, external-change handling, Markdown mutations, printing, and save dialogs solve writing problems. The reading app should not carry these paths forward merely because it is a fork.
- `omawrite.pro` builds C++17 with Qt Quick plus widgets, printing, concurrent, dialogs, and D-Bus modules. Keeping QML while introducing Rust still means a mixed build and a language boundary.
- `LICENSE` is MIT with a retained copyright/permission notice; the bundled font has its separate OFL notice. This does not determine the licenses of a replacement toolkit.

The fork can retain useful QML and history while replacing the document model. A full toolkit rewrite would mainly reuse visual decisions and attribution, not the renderer implementation.

## Parsing is separate from layout

The Rust [`epub` crate](https://docs.rs/epub/latest/epub/) exposes metadata, resources, and reading-order navigation. Its documented chapter operations do not provide line layout, text selection, a visible reading position, or EPUB pagination. Another research ticket evaluates parser candidates.

For normalized prose, the app must preserve meaningful structure such as headings, emphasis, paragraphs, scene breaks, and links while specifying what happens to book CSS and unsupported content. Flattening XHTML to a string risks losing reading meaning even in fiction. For browser layout, the app still needs controlled resource loading and a stable correspondence between visible text and speedreading tokens. Neither strategy should persist a pixel scroll offset as its sole reading location.

## Shortlist

### Qt Quick/QML and Rust

[Qt supports Linux, macOS, Windows, Android, and iOS](https://doc.qt.io/qt-6/supported-platforms.html). Qt Quick provides [accessibility metadata](https://doc.qt.io/qt-6/accessible-qtquick.html). This is the strongest continuity with the inspected app. Normalized styled prose could use Qt text components, but [`Text.RichText` supports a limited HTML subset](https://doc.qt.io/qt-6/qml-qtquick-text.html), not EPUB CSS fidelity.

[CXX-Qt](https://github.com/KDAB/cxx-qt) lets Rust implement QObject classes exposed to QML and C++. Qt also announced [Qt Bridge for Rust 0.3 beta on September 23, 2026](https://www.qt.io/blog/qt-bridge-for-rust-0.3-cxx-qt-compatibility-and-soundness), with shared CXX-Qt foundations. The latter is explicitly beta. Compare pinned examples before choosing bindings; Rust support does not eliminate Qt libraries, QML, or build integration.

For HTML rendering, Qt WebEngine adds Chromium and packaging requirements. Its [deployment documentation](https://doc.qt.io/QT-6/qtwebengine-deploying.html) limits it to desktop Windows, Linux, and macOS. The [platform notes](https://doc.qt.io/qt-6.10/qtwebengine-platform-notes.html) also describe Mac App Store incompatibility. Therefore Qt's overall mobile support is not a promise that a WebEngine reader ports unchanged.

[Qt licensing](https://doc.qt.io/qt-6.10/licensing.html) varies by module and includes LGPL, GPL, and commercial options. Qt's [own distribution guidance](https://www.qt.io/development/open-source-lgpl-obligations) identifies app-store compatibility as a consideration. Revisit the exact modules and distribution method before public or mobile release; do not assume Omawrite's MIT license covers Qt.

### Tauri and Rust

[Tauri supports desktop and mobile applications with a web frontend](https://v2.tauri.app/start/). It is an installed application with Rust application logic, but its UI is HTML/CSS rendered in an embedded browser. This distinction needs the user's agreement on what “native” means.

[Tauri uses system webviews](https://v2.tauri.app/reference/webview-versions/): WebKitGTK on Linux, WKWebView on Apple platforms, WebView2 on Windows, and Android WebView. This provides browser text layout while leaving differences between platform engines and versions. It does not supply an EPUB reading system automatically.

This route replaces Omawrite's QML. It adds web UI tooling and Rust/frontend communication, but avoids constructing HTML layout from GUI labels. The [official repository](https://github.com/tauri-apps/tauri) uses MIT or Apache-2.0 licensing. Linux requires webview system dependencies and packaging validation. Accessibility depends on semantic frontend markup and the webview/assistive-technology combination; no screen-reader compatibility test was performed here.

### Slint and Rust

[Slint's current platform documentation](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/general/) supports Android and iOS, with Rust required for iOS. The [iOS guide](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/ios/) documents Winit/Skia and a macOS/Xcode build. Older indexed documentation says iOS is planned; use the current detailed guide rather than repeating that stale limitation. This research has not established parity across mobile features or release tags.

Slint has [text wrapping and font controls](https://docs.slint.dev/latest/docs/slint/reference/elements/text/), [styled text parsed from Markdown](https://docs.slint.dev/latest/docs/slint/reference/property-types/strings/), and [accessibility roles/properties](https://docs.slint.dev/latest/docs/slint/reference/common/). Those APIs are promising for normalized prose and a custom playback view. They are not evidence of an EPUB HTML/CSS engine. Book selection, hit testing, long-chapter layout, and screen-reader behavior need a proof before adopting it.

Its [license choices](https://github.com/slint-ui/slint/blob/master/LICENSE.md) include GPLv3, a royalty-free option with conditions, and commercial licensing. Inspect the chosen terms, including attribution. It is not simply an MIT Rust GUI. Omawrite UI code would be rewritten in Slint's declarative language.

## Other candidates screened

| Candidate | Evidence and reason not to prioritize now |
| --- | --- |
| egui | The [official README](https://github.com/emilk/egui/blob/main/README.md) documents Rust immediate-mode UI and eframe support for Linux, macOS, Windows, web, and Android. iOS is not in that support list. MIT/Apache-2.0 licensing is convenient. It offers no demonstrated advantage here over Qt or Slint for sustained book reading; full text interaction and mobile coverage remain work to prove. Suitable for a disposable pacing experiment. |
| Iced | The [project README](https://github.com/iced-rs/iced) describes the Rust GUI as experimental and uses MIT licensing. The [accessibility issue](https://github.com/iced-rs/iced/issues/552) tracks unresolved work. Do not assume complete accessible text reading or production mobile parity from “cross-platform.” This investigation did not verify a released all-platform solution. |
| Flutter plus optional Rust | Flutter [documents all five target platforms](https://docs.flutter.dev/reference/supported-platforms) and [framework accessibility support](https://docs.flutter.dev/ui/accessibility). Its [license](https://github.com/flutter/flutter/blob/master/LICENSE) is BSD-style. It merits reconsideration if mobile becomes primary, but introduces Dart and optional Rust interop while replacing the whole Omawrite UI. Platform support still does not supply an EPUB renderer. |

## Evidence required before the architecture decision

A small comparison should render the same representative chapter in Qt's normalized text view and a browser view. Include italics, scene breaks, long paragraphs, footnote links, and font changes. Assess whether normalization is acceptable before adding a third toolkit experiment.

Verify keyboard-only opening, switching modes, stopping, and returning to the precise finished passage. Test selection-to-token mapping across Unicode and inline markup. Check font fallback, fractional scaling, scrolling, and sustained pacing on the actual Linux desktop. Use Orca to inspect conventional text and control names, not just whether a toolkit has accessibility APIs. Measure startup and idle/playback resource use only if it affects comfort or laptop use.

Future ports require touch interaction, file-provider access, lifecycle/pause behavior, mobile screen-reader tests, signing, and platform build tools. A reusable Rust content/position/playback core reduces future rewrite work; it cannot make these platform tasks disappear. Keep the first experiment bounded to Linux, with mobile feasibility documented rather than declared complete.
