#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
./build.sh
if [[ "$(uname -s)" == Linux ]]; then
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
fi
exec ./src-tauri/target/debug/quiet-reader-tauri "$@"
