# Reader reliability work

- [x] Read the foundational principles and inspect the current implementations.
- [x] Confirm both native frontends are in scope; publisher CSS remains ignored.
- [x] Make per-book ownership and durable recovery independent of other open books.
- [x] Replace retained whole-book data with bounded disk-backed text windows.
- [x] Keep image bytes outside text windows; retain only nearby images.
- [x] Exercise navigation, playback, chapter recovery and concurrent processes.
- [x] Measure retained memory on increasing book sizes and repeated traversal.
- [x] Review the integrated changes and record remaining limitations.

Done means both native readers can open different books concurrently, reject the same book, reopen stopped after interrupted writes, traverse a large publication without retaining its full text/images, and preserve source-based positions and current typography.

Work is isolated across persistence, core disk storage, Qt integration and Tauri integration. Existing native suites remain regression gates. New checks must exercise actual persistence/process boundaries and disk windows, rather than only inspect configuration constants.
