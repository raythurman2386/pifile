#!/usr/bin/env bash
# Remove a Pifile user install from ~/.local.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"

rm -f "$PREFIX/bin/pifile"
rm -f "$PREFIX/share/applications/pifile.desktop"
rm -f "$PREFIX/share/icons/hicolor/scalable/apps/pifile.svg"
rm -f "$PREFIX/share/icons/hicolor/128x128/apps/pifile.png"
rm -rf "$PREFIX/share/licenses/pifile"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Removed pifile from $PREFIX"
