#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/web"
exec python -m http.server 18772 --bind 127.0.0.1
