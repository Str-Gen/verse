#!/usr/bin/env bash
set -euo pipefail
exec "$(dirname "$0")/../integration/launch.sh" tauri "$@"
