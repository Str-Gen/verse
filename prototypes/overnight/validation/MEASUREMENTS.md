# Sequential Linux measurements

`measure.py` needs Python 3 and Linux `/proc`. Run only synthetic books. The report records command arguments, so never pass secrets or private publication text. Application stdout and stderr are discarded. Build artifacts must already exist before measurement. Do not point the tool at another worker's mutable build.

From this directory, a self-contained demonstration is:

```sh
python3 measure.py --output .scratch/measurement-demo.json --label synthetic-python --runs 2 --warmups 1 --duration 0.3 --package measure.py -- python3 measure.py _demo
```

Every launch receives a unique `READER_MEASURE_READY_FILE` path. A frontend should create that file only after its synthetic book is laid out and the first page is displayed. Use the same readiness definition for both frontends. Time starts before `Popen` and ends when the tool observes the marker. The polling interval, process creation and `/proc` scans contribute to this measured latency. A marker alone cannot prove a rendered frame reached the display; retain UI evidence separately.

If no marker integration exists, `--ready-tcp 18773` probes only `127.0.0.1`. A port that already accepts connections is an error. TCP readiness measures listener startup, not UI rendering or book readiness. Do not compare TCP readiness for one frontend with first-page readiness for the other. The tool cannot constrain what interfaces a launched application binds. Configure applications to use loopback explicitly.

For integration, stop overnight builds and close previous prototypes. Record build mode, commit, fixture, display scale, compositor, window dimensions and relevant system activity with the results. Use matching release builds, the same synthetic book and isolated empty application storage for the first launch. Prepare all dependencies and assets in advance. Run one frontend at a time, never overlapping compilation or measurement. Run Qt, then Tauri, then reverse the order for a second batch to expose order effects. Keep storage policy identical. Do not delete or reuse an existing prototype's storage.

```sh
python3 measure.py --output .scratch/qt-first.json --label 'qt RELEASE COMMIT FIXTURE' --warmups 0 --runs 1 --duration 5 --package /absolute/path/to/qt-package -- /absolute/path/to/qt-executable --book-json /absolute/path/to/synthetic.json
python3 measure.py --output .scratch/qt-primed.json --label 'qt RELEASE COMMIT FIXTURE' --warmups 1 --runs 5 --duration 5 --package /absolute/path/to/qt-package -- /absolute/path/to/qt-executable --book-json /absolute/path/to/synthetic.json
```

Replace the executable paths and flags with each frontend's documented invocation. Repeat for Tauri with the same settings. These are procedure templates, not claims that either frontend is available or supports the readiness marker.

The first run has fresh processes and uncontrolled OS caches. Subsequent runs also have fresh processes, but earlier launches have primed caches. `--warmups 1` excludes that first launch from the measured `runs` array while retaining it under `warmups`. The tool never drops kernel caches, resets app storage or claims a cold filesystem cache. A genuinely cold machine run requires a separately documented fresh boot and identical workload preparation, which this tool does not perform. Report first launch and primed launches separately; compare medians and ranges from the `runs` entries, not a selected fastest run.

The report records UTC time, kernel, architecture, Python version, CPU count, total RAM, uptime and starting load averages. These describe the run context; they do not establish an idle machine.

Memory samples include descendants and process-group members. RSS sums can count shared pages more than once. PSS apportions shared pages through `smaps_rollup`. A sample has a null aggregate when any selected process cannot be read, including races with exiting processes. The report keeps readable process counts so missing coverage is visible. Sampled peaks are lower bounds on instantaneous peaks; short-lived processes may escape sampling. Processes that daemonize into another session after their parent exits may escape tracking and cleanup. Launch foreground applications. The tool terminates its process group after each run, including failed readiness checks.

Package footprint sums `lstat` apparent and allocated bytes for exactly the selected `--package` paths, including directory metadata. Hardlinks count once; symlinks themselves count, and their targets are not followed. Installed Qt/WebKit/system libraries are excluded unless supplied as additional package paths. Report the selected artifact footprint and separately document installed dependencies, versions and installation footprint. These numbers are not a self-contained distribution size, download size or platform portability result.

No Qt/Tauri comparison is established by the synthetic Python demo. Linux results do not verify Windows or macOS behavior.
