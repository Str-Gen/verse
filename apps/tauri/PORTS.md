# Porting plan

Windows, macOS, Android and iOS are **untested**. These are implementation steps based on official Tauri v2 documentation reviewed on 2026-10-21, not evidence that this prototype builds or behaves correctly on those platforms. Historical Linux results are kept locally in ignored `docs/private/tauri/REPORT.md`. No installers, signing identities or mobile projects were produced for this document.

Run the commands below from this component's directory with a Tauri v2 Cargo CLI installed. Before producing installers, set `bundle.active` to `true` in the relevant platform configuration and supply platform icons; the prototype currently sets it to `false` and builds only an executable. They assume the frontend has been prepared as specified in the local README. Keep `CARGO_BUILD_JOBS=2`, or `$env:CARGO_BUILD_JOBS="2"` in PowerShell. The root workspace owns Cargo profiles and the lockfile.

## Linux dependency boundary

Tauri's Arch dependency list includes `webkit2gtk-4.1`, `base-devel`, `openssl`, `appmenu-gtk-module`, `libappindicator-gtk3`, `librsvg`, `xdotool`, `curl`, `wget` and `file`. Debian uses `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` and `libxdo-dev`. These are system dependency requirements, not permission to install privileged packages in this overnight task. [Official prerequisites](https://v2.tauri.app/start/prerequisites/)

An executable's file size excludes these shared libraries and WebKit's subprocesses. Record executable, bundle, dependency and process-tree measurements separately. Browser rendering alone does not validate GTK linking, the WebKitGTK engine, native IPC or native persistence.

## Windows

Use Microsoft's C++ desktop build tools, an MSVC Rust toolchain and Edge WebView2. [Official prerequisites](https://v2.tauri.app/start/prerequisites/)

Replace Bash launch wrappers with a PowerShell equivalent or invoke the Cargo CLI directly. Preserve arguments as distinct arguments and use platform paths through Rust's path APIs. Run `cargo tauri dev` first, then `cargo tauri build --bundles nsis` for an installer. The Windows installer normally downloads the WebView2 bootstrapper if needed. For offline installation, set `bundle.windows.webviewInstallMode.type` to `offlineInstaller`; measure its size separately. Add `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` to desktop `main.rs` so a packaged release does not leave a console window. [Windows installers](https://v2.tauri.app/distribute/windows-installer/), [entry-point example](https://v2.tauri.app/start/migrate/from-tauri-1/#preparing-for-mobile)

Recheck selection and source conversion in WebView2, including non-BMP characters, plus reflow at Windows display scaling settings. Test stopped resume after killing and reopening the process. These checks remain required even if Linux passes.

## macOS

Use Xcode command-line tools for desktop development, or full Xcode when also targeting iOS. [Official prerequisites](https://v2.tauri.app/start/prerequisites/)

Run `cargo tauri dev` on a Mac, then `cargo tauri build --bundles app`. Add the application's `.icns` icon and bundle metadata before packaging. Build and test separately on each intended architecture. Distribution outside this local experiment requires a separate signing and notarization workflow; no distribution is authorized here. [macOS application bundle](https://v2.tauri.app/distribute/macos-application-bundle/)

Use an application-specific writable data directory instead of assuming the executable or bundle is writable. Recheck pagination and selection in the system WebKit, including trackpad selection and keyboard navigation. Confirm that increasing font size keeps the same source passage.

## Shared mobile code changes

Ensure `src-tauri/Cargo.toml` has a `[lib]` target with a distinct library name and `crate-type = ["staticlib", "cdylib", "rlib"]`. Put app startup in a public `run()` function in `src-tauri/src/lib.rs`, marked `#[cfg_attr(mobile, tauri::mobile_entry_point)]`. Desktop `main.rs` calls that library function. If the prototype already uses this structure, retain it. [Tauri mobile preparation](https://v2.tauri.app/start/migrate/from-tauri-1/#preparing-for-mobile)

Required application work, not a claim from the framework docs:

- Replace the desktop `--book-json` startup path with a picker/import flow that copies a user-selected JSON export into private app storage. Keep the same version-1 JSON adapter. Do not add another EPUB parser in JavaScript.
- Gate desktop-only arguments and window APIs. Link the Rust content core as a library when EPUB import is added; do not assume a desktop CLI subprocess will exist on mobile.
- Persist source anchor, revision and speed when the app becomes inactive. Resume stopped after suspension or termination. Background timer throttling must never make playback race forward on return.
- Add touch passage selection and accessible on-screen page controls. Apply safe-area insets, test orientation changes and preserve source position across them.
- Keep application files and imported books in an app-specific writable location. Use separate synthetic storage for development and tests.

## Android

Install Android Studio, its SDK platform/build/platform/command-line tools and side-by-side NDK. Set `JAVA_HOME`, `ANDROID_HOME` and `NDK_HOME` to their actual local installations. Add Rust targets `aarch64-linux-android`, `armv7-linux-androideabi`, `i686-linux-android` and `x86_64-linux-android` as needed. [Official prerequisites](https://v2.tauri.app/start/prerequisites/)

After the shared mobile changes, run `cargo tauri android init`, review generated `src-tauri/gen/android`, then `cargo tauri android dev`. Use `cargo tauri android build --apk` for a local installable artifact. A future store bundle uses `cargo tauri android build --aab`, with release signing handled separately. [CLI reference](https://v2.tauri.app/reference/cli/), [Google Play build guide](https://v2.tauri.app/distribute/google-play/)

Use an emulator or a USB-connected test device with an explicitly configured loopback forwarding arrangement. Do not expose the development server on a public interface to make mobile development work. Test file import permissions, touch selection and process death on a real device before claiming support.

## iOS

Build on macOS with full Xcode and CocoaPods. Install Rust targets `aarch64-apple-ios`, `x86_64-apple-ios` and `aarch64-apple-ios-sim` as needed. [Official prerequisites](https://v2.tauri.app/start/prerequisites/)

Run `cargo tauri ios init`, inspect generated `src-tauri/gen/apple`, then `cargo tauri ios dev` for the simulator or a configured device. `cargo tauri ios build` is the subsequent build command. Configure the bundle identifier, Apple development team and signing for a physical device in the generated Xcode project/configuration. These require a separate Mac/device verification session. [CLI reference](https://v2.tauri.app/reference/cli/)

Test selected-file access, safe areas, selection handles, background suspension and stopped reopening. Keep desktop command-line import behind target-specific code. Do not treat a successful simulator build as proof of physical-device behavior.

## Security and configuration on every target

Tauri merges platform files such as `tauri.windows.conf.json`, `tauri.macos.conf.json`, `tauri.android.conf.json` and `tauri.ios.conf.json` with the main configuration. Put platform bundle settings there instead of weakening shared security settings. [Configuration reference](https://v2.tauri.app/reference/config/)

Keep book data as text and normalized structural values. Never evaluate publisher scripts or insert publisher HTML. Deny remote book resources, frames, objects and navigation. Permit only the application's scripts, styles and necessary IPC origins in the CSP; keep Tauri's CSP rewriting enabled. If local publication images are added, validate their paths and use a narrowly scoped local asset route. A permissive `img-src https:` rule would violate the offline book boundary. [Tauri CSP](https://v2.tauri.app/security/csp/)

Keep capabilities limited to the reader window and the commands it needs. Do not grant shell execution, unrestricted filesystem access or remote origins to solve a porting error. Validate custom Rust command inputs independently of frontend checks. [Tauri capabilities](https://v2.tauri.app/security/capabilities/)

Verify the policy in each packaged engine with synthetic malicious text and remote-resource URLs. Check that they appear as harmless text or explicit unsupported content and produce no network request. Private books and exported text must stay out of screenshots, logs, fixtures and bundles.
