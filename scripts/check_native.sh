#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$here/evidence" "$here/.local"
"$here/export.sh" demo --output "$here/evidence/core-demo.json" --store "$here/.local/identities"
cp "$here/evidence/core-demo.json" "$here/../apps/tauri/web/core-demo.json"
cargo +1.99.0 build --manifest-path "$here/../apps/qt/Cargo.toml" --locked -j 2
for platform in offscreen wayland; do
  for mode in core unicode; do
    input="$here/evidence/core-demo.json"
    if [[ "$mode" == unicode ]]; then input="$here/../apps/tauri/web/unicode.json"; fi
    state=$(mktemp -d "$here/.local/native-$platform-$mode.XXXXXX")
    QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM="$platform" QT_QUICK_BACKEND=software QT_SCALE_FACTOR=1 \
      READER_THEME_FILE="$here/../apps/qt/fixtures/theme-light.toml" READER_STATE_DIR="$state" \
      READER_SMOKE="$mode" READER_SCREENSHOT="$here/evidence/qt-$platform-$mode.png" \
      "$here/../target/debug/verse-qt" --book-json "$input" \
      > "$here/evidence/qt-$platform-$mode.log" 2>&1
    if rg -q 'FAIL|ReferenceError|TypeError|Cannot open' "$here/evidence/qt-$platform-$mode.log"; then cat "$here/evidence/qt-$platform-$mode.log"; exit 1; fi
    rg 'COMPLETE' "$here/evidence/qt-$platform-$mode.log"
  done
done
"$here/../apps/tauri/verify.sh"
