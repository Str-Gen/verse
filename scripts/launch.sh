#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
frontend=$1
shift
case "$frontend" in qt|tauri) ;; *) echo 'Expected qt or tauri' >&2; exit 2;; esac
if [[ "${1:-}" == --demo || "${1:-}" == --epub ]]; then
  mode=$1
  shift
  mkdir -p "$here/.local"
  if [[ "$mode" == --demo ]]; then
    input="$here/../crates/reader-core/fixtures/complex.epub"
  else
    input=$(realpath "${1:?--epub requires a path}")
    shift
  fi
  key=$(printf '%s' "$input" | sha256sum | cut -d' ' -f1)
  output="$here/.local/$key.json"
  "$here/export.sh" export --epub "$input" --output "$output" --store "$here/.local/identities"
  set -- --book-json "$output" "$@"
fi
if [[ "${1:-}" == --book-json ]]; then
  input=$(realpath "${2:?--book-json requires a path}")
  shift 2
  set -- --book-json "$input" "$@"
fi
cd "$here/../apps/$frontend"
if [[ "$frontend" == qt ]]; then
  cargo +1.99.0 build --locked -j 2
  exec "$here/../target/debug/verse-qt" "$@"
else
  ./build.sh
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
  exec "$here/../target/debug/verse-tauri" "$@"
fi
