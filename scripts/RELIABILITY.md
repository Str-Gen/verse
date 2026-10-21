# Native reader reliability

Implemented in both Qt and Tauri on 21 October 2026. Publisher CSS remains ignored; the reader keeps its existing typography and contrast.

## Behavior

Different books can run concurrently, including two instances sharing a frontend state directory. Both frontends use the same per-book OS lock, so opening the same local book twice fails clearly. The OS releases ownership when a process exits or is killed. Concurrent first imports serialize local identity allocation, preventing two identities for the same canonical path.

Each book has independent current, backup and chapter checkpoints. Writes use a temporary sibling, file sync, atomic replacement and directory sync. Playback checkpoints at most every two seconds; explicit pauses and position changes save immediately. Reopening stays stopped. Recovery validates the actual position before accepting a record, tries a usable backup, then an identified chapter start. Same-edition chapter fallback is automatic. Changed editions still require confirmation. With no recoverable chapter, the reader starts at the beginning without inventing an old position. Invalid optional history does not discard an otherwise valid anchor.

Text anchors use source spans. Source-less image and unsupported events use a verified piece, block, kind and text identity within the same book revision. A chapter starting with an image can therefore save both its start and later text positions.

Native text comes from a disk index through reusable buffers. The current window contains at most 512 pieces, including 128 pieces of backward overlap when available. Frontends replace old text, source maps and playback plans. Vocabulary replaces active text while preserving captured sentences in SQLite. Cache files live under the user cache directory, rather than relying on a RAM-backed temporary directory. OS leases protect live caches and permit abandoned-cache cleanup after a crash.

Images are independent of the text buffer. The nearby set contains two previous images, the current image when selected, and two following images. Qt uses one asynchronous decoding worker; Tauri predecodes images and revokes evicted blob URLs. Both limit prepared image edges to 1600 pixels. Missing images retain an explicit placeholder. Decoding is asynchronous, but these tests do not establish a worst-case frame-time guarantee.

## Verification

Run the core and workspace tests documented in the root README. The native checks exercise restart, source selection, notes, bounded text windows, image traversal and checkpoint recovery. Keep run results and profiles in `docs/private/`. Passing process-termination tests does not establish behavior under a physical power interruption.

## Remaining bounds

EPUB export holds one XHTML DOM at a time, with a 16 MiB expanded-entry limit. It does not stream XML node by node within that document. Archive validation and hashing use fixed buffers; normalized output is staged on disk. The aggregate import API remains available for diagnostic commands.

Native indexing decodes individual JSON blocks and image resources as units. Visible text context is limited to 2 MiB, a serialized piece to 16 KiB, and aggregate loaded note records to 1 MiB. Oversized notes show an explicit unsupported marker. Thus normal reading does not retain the whole book, but import peak memory still depends on the largest permitted document or resource. These limits must be considered before claiming constant memory for every EPUB structure.

## Repeating the checks

Run from the repository root after building both native binaries:

```sh
mkdir -p docs/private
python3 scripts/tests/instances.py \
  --qt target/debug/verse-qt \
  --tauri target/debug/verse-tauri \
  --fixture apps/tauri/web/demo.json \
  --output docs/private/reader-instances.json
python3 scripts/tests/recovery.py \
  --binary target/debug/verse-tauri \
  --fixture apps/tauri/web/demo.json \
  --output docs/private/reader-recovery.json
python3 scripts/tests/memory.py \
  --frontend qt --binary target/debug/verse-qt \
  --large-pieces 60000 --output docs/private/reader-memory-qt.json
python3 scripts/tests/memory.py \
  --frontend tauri --binary target/debug/verse-tauri \
  --large-pieces 60000 --output docs/private/reader-memory-tauri.json
```
