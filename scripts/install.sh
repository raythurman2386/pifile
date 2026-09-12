#!/usr/bin/env bash
# Install Pifile into ~/.local (binary, desktop entry, icon). No root.
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

cd "$ROOT"
cargo build --release --locked

# target-dir may be overridden (e.g. shared cache in ~/.cargo/config.toml),
# so ask cargo where the build actually landed instead of assuming target/.
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
[[ -n "$TARGET_DIR" ]] || { echo "install.sh: could not resolve cargo target directory" >&2; exit 1; }

install -Dm755 "$TARGET_DIR/release/pifile" "$PREFIX/bin/pifile"
install -Dm644 "$ROOT/dist/pifile.desktop" "$PREFIX/share/applications/pifile.desktop"
install -Dm644 "$ROOT/LICENSE" "$PREFIX/share/licenses/pifile/LICENSE"
if [[ -f "$ROOT/dist/pifile.svg" ]]; then
  install -Dm644 "$ROOT/dist/pifile.svg" "$PREFIX/share/icons/hicolor/scalable/apps/pifile.svg"
  if command -v rsvg-convert >/dev/null 2>&1; then
    tmp="$(mktemp --suffix=.png)"
    rsvg-convert -w 128 -h 128 "$ROOT/dist/pifile.svg" -o "$tmp"
    install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pifile.png"
    rm -f "$tmp"
  elif command -v magick >/dev/null 2>&1; then
    tmp="$(mktemp --suffix=.png)"
    magick -background none "$ROOT/dist/pifile.svg" -resize 128x128 "$tmp"
    install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pifile.png"
    rm -f "$tmp"
  fi
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed pifile into $PREFIX"
echo "Run it with: pifile"