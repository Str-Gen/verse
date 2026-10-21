# Native reader reliability

Implemented in both Qt and Tauri on 21 October 2026. Publisher CSS remains ignored; the reader keeps its existing typography and contrast.

## Behavior

Different books can run concurrently, including two instances sharing a frontend state directory. Both frontends use the same per-book OS lock, so opening the same local book twice fails clearly. The OS releases ownership when a process exits or is killed. Concurrent first imports serialize local identity allocation, preventing two identities for the same canonical path.

Each book has independent current, backup and chapter checkpoints. Writes use a temporary sibling, file sync, atomic replacement and directory sync. Playback checkpoints at most every two seconds; explicit pauses and position changes save immediately. Reopening stays stopped. Recovery validates the actual position before accepting a record, tries a usable backup, then an identified chapter start. Same-edition chapter fallback is automatic. Changed editions still require confirmation. With no recoverable chapter, the reader starts at the beginning without inventing an old position. Invalid optional history does not discard an otherwise valid anchor.

Text anchors use source spans. Source-less image and unsupported events use a verified piece, block, kind and text identity within the same book revision. A chapter starting with an image can therefore save both its start and later text positions.

Native text comes from a disk index through reusable buffers. The current window contains at most 512 pieces, including 128 pieces of backward overlap when available. Frontends replace old text, source maps and playback plans. Vocabulary replaces active text while preserving captured sentences in SQLite. Cache files live under the user cache directory, rather than relying on a RAM-backed temporary directory. OS leases protect live caches and permit abandoned-cache cleanup after a crash.

Images are independent of the text buffer. The nearby set contains two previous images, the current image when selected, and two following images. Qt uses one asynchronous decoding worker; Tauri predecodes images and revokes evicted blob URLs. Both limit prepared image edges to 1600 pixels. Missing images retain an explicit placeholder. Decoding is asynchronous, but these tests do not establish a worst-case frame-time guarantee.

## Verification

- Core: 38 unit tests, 19 EPUB contract tests and a nine-image window traversal test, including an export with more than 12 MiB of text, streamed archive validation, malformed and semantically invalid checkpoints, concurrent identity creation, and lock release after process termination.
- Qt: 20 model tests, strict Clippy, the complete native smoke/restart/selection/note/window suite, and traversal through all five images in the core fixture with decoded-cache and source-position checks.
- Tauri: 4 Rust tests, 7 JavaScript model tests, strict Clippy, and 72 native WebKit checks. The native suite passes with intact state, truncated primary state, and valid JSON containing an invalid source anchor. It checks stopped reopening, source-less image persistence, bounded windows, and Escape during asynchronous navigation.
- Vocabulary: 5 worker tests, including captured-context retention after text-window replacement.
- Real-process instance checks: Qt with malformed startup state, rejection of the same book in Tauri, concurrent different books including two Tauri processes sharing a scratch root, and reopening after killing the owning Qt process.

The common memory driver calls separate native adapters and verifies 40 distant seeks per launch. This is what an action-trace driver means: a program performs a fixed sequence of reader actions and checks the resulting state. The existing native suites also cover playback and source selection. A general shared action-trace format and a new private real-book review were not added.

Both installed release commands were rebuilt. `verse-qt` passed native window/source-navigation smoke checks; `verse-tauri` passed all 72 native checks through its installed launcher. Memory figures below are from debug builds, not those release binaries.

No physical power interruption was performed. Evidence covers process termination, damaged files, recovery selection and durable-write operations; it does not certify every storage device's power-loss behavior.

## Memory measurement

Final debug binaries ran five alternating trials at each size, each with fresh state and 40 distant seeks. Fixtures contain 5,000 and 60,000 pieces, approximately 0.97 MB and 11.86 MB of normalized JSON. The larger fixture is twelve times the text. They contain no images or large notes. The sampler reads Linux process-tree proportional set size every 50 ms after the native app announces that indexing and startup have completed. Child processes are included. Sampled peaks can miss shorter allocation spikes.

| Native build | Pieces | Reading process-tree peak PSS, median | Range |
| --- | ---: | ---: | ---: |
| qt | 5,000 | 83.0 MiB | 82.9 to 83.3 MiB |
| qt | 60,000 | 83.6 MiB | 83.4 to 83.7 MiB |
| tauri | 5,000 | 412.0 MiB | 363.2 to 454.9 MiB |
| tauri | 60,000 | 430.2 MiB | 416.7 to 450.6 MiB |

These figures measure content scaling in this environment, not improvement against a historical build. Qt ran offscreen with software rendering; Tauri ran through WebKitGTK in the graphical session. They are not a controlled toolkit comparison. Tauri's runtime remains substantial even when retained text is bounded. The host-process medians were 151.2 and 152.1 MiB; most variation was in WebKit child processes. Qt host-process medians were 66.0 and 66.4 MiB. Image-heavy books, long sessions measured in hours, and giant notes need separate memory measurements. The application and OS may retain allocator arenas or decoded graphics caches; five application image entries alone do not prove a five-image process-memory ceiling.

Raw results include binary hashes, fixture sizes, timing, per-process samples, native action results and thread counts in [evidence/reliability](evidence/reliability). The first Qt measurement attempt is retained separately: its driver bypassed the actual chapter UI action and failed a selection check. The driver now uses the actual action; the assertion was retained.

## Remaining bounds

EPUB export holds one XHTML DOM at a time, with a 16 MiB expanded-entry limit. It does not stream XML node by node within that document. Archive validation and hashing use fixed buffers; normalized output is staged on disk. The aggregate import API remains available for diagnostic commands.

Native indexing decodes individual JSON blocks and image resources as units. Visible text context is limited to 2 MiB, a serialized piece to 16 KiB, and aggregate loaded note records to 1 MiB. Oversized notes show an explicit unsupported marker. Thus normal reading does not retain the whole book, but import peak memory still depends on the largest permitted document or resource. These limits must be considered before claiming constant memory for every EPUB structure.

## Repeating the checks

Run from the repository root after building both native binaries:

```sh
python3 prototypes/overnight/integration/tests/instances.py \
  --qt prototypes/overnight/qt/target/debug/overnight-qt-reader \
  --tauri prototypes/overnight/tauri/src-tauri/target/debug/quiet-reader-tauri \
  --fixture prototypes/overnight/tauri/web/demo.json \
  --output /tmp/reader-instances.json
python3 prototypes/overnight/integration/tests/recovery.py \
  --binary prototypes/overnight/tauri/src-tauri/target/debug/quiet-reader-tauri \
  --fixture prototypes/overnight/tauri/web/demo.json \
  --output /tmp/reader-recovery.json
python3 prototypes/overnight/integration/tests/memory.py \
  --frontend qt --binary prototypes/overnight/qt/target/debug/overnight-qt-reader \
  --large-pieces 60000 --output /tmp/reader-memory-qt.json
python3 prototypes/overnight/integration/tests/memory.py \
  --frontend tauri --binary prototypes/overnight/tauri/src-tauri/target/debug/quiet-reader-tauri \
  --large-pieces 60000 --output /tmp/reader-memory-tauri.json
```

The focused independent GPT-6 Astra review found invalid-anchor recovery, missing-image placeholders and source-less event persistence issues. All three were fixed and checked before completion. The review did not independently rerun the tests. Memory and power-loss limitations above remain explicit.
