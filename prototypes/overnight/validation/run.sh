#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
export PYTHONDONTWRITEBYTECODE=1
python3 -m unittest discover -s tests -v
python3 validate.py fixtures --output .scratch/fixtures
python3 demo.py --output .scratch/demo.json
python3 frontend.py plan --book-json .scratch/demo.json --output .scratch/ui-plan.json
python3 measure.py --output .scratch/measurement-demo.json --label synthetic-python --runs 2 --warmups 1 --duration 0.3 --package measure.py -- python3 measure.py _demo
printf '%s\n' 'Validation tools exercised. Native core/frontend comparison is pending until real adapters run.'
