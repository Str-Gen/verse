#!/usr/bin/env bash
set -euo pipefail
component_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p "$component_dir/../../target"
# This standalone native probe does not run Cargo. Run after other builds finish.
read -r -a qt_flags <<< "$(pkg-config --cflags --libs Qt6Gui)"
"${CXX:-c++}" -fPIC "$component_dir/tests/whitespace.cpp" \
    "${qt_flags[@]}" -o "$component_dir/../../target/whitespace-probe"
QT_QPA_PLATFORM=offscreen "$component_dir/../../target/whitespace-probe"
