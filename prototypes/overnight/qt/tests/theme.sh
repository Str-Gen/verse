#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .local/theme-test-build
qt_libexec=$(pkg-config --variable=libexecdir Qt6Core)
"$qt_libexec/moc" native/theme.h -o .local/theme-test-build/moc_theme.cpp
c++ -std=c++17 -fPIC -Wall -Wextra -Werror \
  $(pkg-config --cflags Qt6Core Qt6Gui Qt6DBus) -Inative \
  native/theme.cpp native/theme_test.cpp .local/theme-test-build/moc_theme.cpp \
  $(pkg-config --libs Qt6Core Qt6Gui Qt6DBus) \
  -o .local/theme-test-build/theme_test
QT_QPA_PLATFORM=offscreen .local/theme-test-build/theme_test
