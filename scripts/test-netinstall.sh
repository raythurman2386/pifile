#!/usr/bin/env bash
set -euo pipefail

# Local test harness for the pifile netinstaller's integrity + authenticity
# checks.
#
# Builds a fake release (tarball + install.sh + checksums.txt + .sig) in a
# local directory and runs scripts/netinstall.sh against it using the local
# mirror mode (PIFILE_RELEASE_BASE_URL pointing at a filesystem path).
# Verifies that:
#   1. a clean, correctly-signed release installs successfully;
#   2. a tampered tarball is refused (checksum mismatch);
#   3. a tampered checksums.txt is refused (signature verification fails);
#   4. a missing signature is refused (fail closed).
#
# No network access is required. Requires: bash, openssl, sha256sum, tar.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NETINSTALL_SH="$ROOT/scripts/netinstall.sh"
SIGN_SH="$ROOT/scripts/sign-release.sh"

VERSION="0.1.0"
VERSION_TAG="v$VERSION"

# Detect the triple the same way netinstall.sh does (Linux-only here).
arch="$(uname -m)"
case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *) echo "unsupported arch: $arch" >&2; exit 1 ;;
esac
TRIPLE="${arch}-unknown-linux-gnu"
ARTIFACT="pifile-${VERSION}-${TRIPLE}.tar.gz"
STAGE_NAME="pifile-${VERSION}-${TRIPLE}"

WORK="$(mktemp -d)"
RELEASE_ROOT="$WORK/release"
RELEASE_DIR="$RELEASE_ROOT/$VERSION_TAG"
PREFIX_DIR="$WORK/prefix"
KEYS="$WORK/keys"

cleanup() {
    rm -rf "$WORK"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

pass() {
    echo "PASS: $1"
}

# --- helpers ---------------------------------------------------------------

# Build the release tarball: fake binary + install.sh + desktop file + icon.
make_release_tarball() {
    local stage="$WORK/$STAGE_NAME"
    rm -rf "$stage"
    mkdir -p "$stage"

    cat > "$stage/pifile" <<'EOF'
#!/usr/bin/env bash
echo "pifile 0.1.0 (fake)"
EOF
    chmod +x "$stage/pifile"

    cat > "$stage/install.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
HERE="\$(cd "\$(dirname -- "\$0")" && pwd)"
PREFIX="\${PREFIX:-\$HOME/.local}"
mkdir -p "\$PREFIX/bin"
install -m 755 "\$HERE/pifile" "\$PREFIX/bin/pifile"
echo "Installed pifile to \$PREFIX/bin/pifile"
EOF
    chmod +x "$stage/install.sh"

    tar -czf "$RELEASE_DIR/$ARTIFACT" -C "$WORK" "$STAGE_NAME"
}

write_checksums() {
    # checksums.txt uses two spaces between hash and name (matches netinstall).
    local hash
    hash="$(sha256sum "$RELEASE_DIR/$ARTIFACT" | awk '{print $1}')"
    printf '%s  %s\n' "$hash" "$ARTIFACT" > "$RELEASE_DIR/checksums.txt"
}

run_netinstall() {
    # Run netinstall.sh against the local release directory. Returns its exit code.
    PIFILE_RELEASE_BASE_URL="$RELEASE_ROOT" \
        PREFIX="$PREFIX_DIR" \
        bash "$NETINSTALL_SH_PATCHED" "$VERSION_TAG" >"$WORK/out.log" 2>&1
}

# --- setup: generate a keypair and patch netinstall.sh's pinned key ---------

mkdir -p "$RELEASE_DIR" "$PREFIX_DIR" "$KEYS"

# Generate a fresh keypair. The harness patches a temp copy of netinstall.sh to
# pin this generated public key, so the test is self-contained (it does not
# depend on the committed public key matching a secret we can produce here).
openssl genpkey -algorithm ED25519 -out "$KEYS/secret.pem" 2>/dev/null
openssl pkey -in "$KEYS/secret.pem" -pubout -out "$KEYS/public.pem" 2>/dev/null

NETINSTALL_SH_PATCHED="$WORK/netinstall.sh"
python3 - "$NETINSTALL_SH" "$KEYS/public.pem" "$NETINSTALL_SH_PATCHED" <<'PY'
import sys, re
src, pub, dst = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(src).read()
pubkey = open(pub).read().strip()
patched = re.sub(
    r"-----BEGIN PUBLIC KEY-----\n.*?\n-----END PUBLIC KEY-----",
    pubkey,
    text,
    count=1,
    flags=re.S,
)
open(dst, "w").write(patched)
PY

# --- case 1: clean install succeeds ---------------------------------------

make_release_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null

rc=0
run_netinstall || rc=$?
if [[ $rc -ne 0 ]]; then
    cat "$WORK/out.log" >&2
    fail "clean install should succeed"
fi
[[ -x "$PREFIX_DIR/bin/pifile" ]] || fail "pifile binary not installed"
"$PREFIX_DIR/bin/pifile" | grep -q "pifile 0.1.0 (fake)" || fail "installed binary does not run"
grep -q "Signature OK" "$WORK/out.log" || fail "expected 'Signature OK' in output"
grep -q "Checksum OK" "$WORK/out.log" || fail "expected 'Checksum OK' in output"
grep -q "Installed pifile to $PREFIX_DIR/bin/pifile" "$WORK/out.log" || fail "expected install.sh to report install"
pass "case 1: clean signed release installs"

# --- case 2: tampered tarball is refused -----------------------------------

rm -rf "$RELEASE_DIR" "$PREFIX_DIR"
mkdir -p "$RELEASE_DIR" "$PREFIX_DIR"
make_release_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null
# Tamper with the tarball AFTER checksums were computed.
printf 'evil payload\n' >> "$RELEASE_DIR/$ARTIFACT"

rc=0
run_netinstall || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "tampered tarball should be refused"
fi
grep -q "checksum mismatch" "$WORK/out.log" || fail "expected checksum mismatch message"
[[ ! -e "$PREFIX_DIR/bin/pifile" ]] || fail "tampered tarball must not be installed"
pass "case 2: tampered tarball refused (checksum mismatch)"

# --- case 3: tampered checksums.txt is refused -----------------------------

rm -rf "$RELEASE_DIR" "$PREFIX_DIR"
mkdir -p "$RELEASE_DIR" "$PREFIX_DIR"
make_release_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null
# Tamper with checksums.txt AFTER signing (signature will no longer verify).
printf 'deadbeef  %s\n' "$ARTIFACT" >> "$RELEASE_DIR/checksums.txt"

rc=0
run_netinstall || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "tampered checksums.txt should be refused"
fi
grep -q "signature verification FAILED" "$WORK/out.log" || fail "expected signature failure message"
[[ ! -e "$PREFIX_DIR/bin/pifile" ]] || fail "tarball must not be installed on bad signature"
pass "case 3: tampered checksums.txt refused (bad signature)"

# --- case 4: missing signature is refused (fail closed) --------------------

rm -rf "$RELEASE_DIR" "$PREFIX_DIR"
mkdir -p "$RELEASE_DIR" "$PREFIX_DIR"
make_release_tarball
write_checksums
# Intentionally do NOT sign — no checksums.txt.sig present.

rc=0
run_netinstall || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "missing signature should be refused"
fi
grep -q "without a release signature" "$WORK/out.log" || fail "expected missing-signature message"
[[ ! -e "$PREFIX_DIR/bin/pifile" ]] || fail "tarball must not be installed without a signature"
pass "case 4: missing signature refused (fail closed)"

echo ""
echo "All netinstaller integrity/authenticity tests passed."