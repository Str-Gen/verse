#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [ "$#" -eq 0 ]; then
  set -- demo --output scratch/demo.json
fi
exec cargo run -p reader-cli --quiet --locked -j 2 -- "$@"
