#!/usr/bin/env bash
# Package Pifile into a self-contained Linux tarball for releases.
# Usage: scripts/package-linux.sh [target-triple]
# Env overrides: TARGET (target triple), PREBUILT_BIN (skip the cargo build),
# OUT_DIR (output directory, default: <cargo target dir>/package).
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
ARCH="${ARCH:-x86_64}"
TARGET="${TARGET:-${1:-${ARCH}-unknown-linux-gnu}}"
# target-dir may be overridden (e.g. shared cache in ~/.cargo/config.toml),
# so ask cargo where the build landed instead of assuming target/.
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
TARGET_DIR="${TARGET_DIR:-target}"
OUT_DIR="${OUT_DIR:-${TARGET_DIR}/package}"
NAME="pifile"
BIN="${PREBUILT_BIN:-${TARGET_DIR}/${TARGET}/release/${NAME}}"
STAGE="${OUT_DIR}/${NAME}-${VERSION}-${TARGET}"

if [[ ! -x "$BIN" ]]; then
  cargo build --release --locked --target "$TARGET"
  BIN="${TARGET_DIR}/${TARGET}/release/${NAME}"
fi
[[ -x "$BIN" ]] || { echo "package-linux: missing binary at $BIN (build --release first)" >&2; exit 1; }

rm -rf "$STAGE"
mkdir -p "$STAGE"

install -Dm755 "$BIN" "$STAGE/${NAME}"
install -Dm644 "dist/${NAME}.desktop" "$STAGE/${NAME}.desktop"
install -Dm644 "dist/${NAME}.svg" "$STAGE/${NAME}.svg"
install -Dm644 "LICENSE" "$STAGE/LICENSE"

cat > "$STAGE/install.sh" << 'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-$HOME/.local}"
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
install -Dm755 "$HERE/pifile" "$PREFIX/bin/pifile"
install -Dm644 "$HERE/pifile.desktop" "$PREFIX/share/applications/pifile.desktop"
install -Dm644 "$HERE/LICENSE" "$PREFIX/share/licenses/pifile/LICENSE"
if command -v rsvg-convert >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"; rsvg-convert -w 128 -h 128 "$HERE/pifile.svg" -o "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pifile.png"; rm -f "$tmp"
fi
install -Dm644 "$HERE/pifile.svg" "$PREFIX/share/icons/hicolor/scalable/apps/pifile.svg"
update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
echo "Installed pifile into $PREFIX"
INSTALL
chmod +x "$STAGE/install.sh"

tar -czf "${OUT_DIR}/${NAME}-${VERSION}-${TARGET}.tar.gz" -C "$OUT_DIR" "${NAME}-${VERSION}-${TARGET}"
echo "Packaged ${OUT_DIR}/${NAME}-${VERSION}-${TARGET}.tar.gz"