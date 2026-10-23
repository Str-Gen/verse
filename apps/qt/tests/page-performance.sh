#!/usr/bin/env bash
set -euo pipefail
component=$(cd "$(dirname "$0")/.." && pwd)
input=$(realpath "${1:?Usage: page-performance.sh BOOK CHAPTER_INDEX}")
chapter=${2:?Supply the zero-based chapter index from the contents list}
[[ "$chapter" =~ ^[0-9]+$ ]]
reader_binary=${READER_BINARY:-$component/../../target/release/verse-qt}
if [[ -z "${READER_BINARY:-}" ]]; then
  cargo build --manifest-path "$component/Cargo.toml" --release --locked -j 2
fi
state=$(mktemp -d)
trap 'rm -rf "$state"' EXIT
flag=--book-json
if [[ "${input,,}" == *.epub ]]; then flag=--epub; fi
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM="${READER_TEST_PLATFORM:-offscreen}" \
  QT_QUICK_BACKEND="${READER_TEST_BACKEND:-software}" QT_SCALE_FACTOR=1 \
  READER_THEME_FILE="$component/fixtures/theme-light.toml" READER_STATE_DIR="$state" \
  READER_SMOKE=page-performance READER_PERFORMANCE_CHAPTER="$chapter" \
  "$reader_binary" "$flag" "$input"
