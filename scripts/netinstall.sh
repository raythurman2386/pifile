#!/usr/bin/env bash
set -euo pipefail

# Netinstaller for Pifile: download the prebuilt release tarball, verify it,
# and run the bundled install.sh. No root, no compiler.
#
# Fails closed on integrity and authenticity: the tarball's SHA-256 must match
# a checksums.txt entry, and checksums.txt must carry a signature that verifies
# against the pinned Ed25519 public key below. A missing or bad signature
# refuses the install.

REPO="raythurman2386/pifile"
# Base URL for release artifacts. Overridable so the installer can be tested
# against a local mirror (a filesystem path or `python3 -m http.server`)
# without hitting GitHub.
DEFAULT_RELEASE_BASE_URL="https://github.com/$REPO/releases/download"
RELEASE_BASE_URL="${PIFILE_RELEASE_BASE_URL:-$DEFAULT_RELEASE_BASE_URL}"
VERSION=""

# Pinned Ed25519 public key (PEM) used to verify the release signature. This is
# the root of trust: it must match the key used by scripts/sign-release.sh and
# the committed pifile-signing-key.pub. A release whose checksums.txt.sig does
# not verify against this key is refused.
SIGNING_PUBLIC_KEY='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEApCLGI7S2DBGC8NrKjrJA+B9AbRI6TGRbstTdQ9qSqnI=
-----END PUBLIC KEY-----'

usage() {
    cat <<EOF
Usage: $0 [VERSION]

Install Pifile from a prebuilt GitHub Release. Verifies the tarball's
SHA-256 against a checksums.txt signed with the pinned Ed25519 key — a
missing or bad signature refuses the install.

Arguments:
  VERSION  Install a specific release tag (default: latest)

Environment:
  PIFILE_RELEASE_BASE_URL  Override the release artifact base URL
  PREFIX                   Install prefix passed to the bundled install.sh
                           (default: \$HOME/.local)

Examples:
  $0
  $0 v0.1.0
  PREFIX=/tmp/test $0
EOF
    exit 0
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
fi

if [[ $# -gt 1 ]]; then
    echo "Error: too many arguments (expected at most one VERSION)" >&2
    exit 1
fi

if [[ $# -eq 1 ]]; then
    VERSION="$1"
fi

detect_triple() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64)  echo "aarch64-unknown-linux-gnu" ;;
        *)
            echo "Error: unsupported architecture: $arch (prebuilt binaries are x86_64 and aarch64 Linux)" >&2
            exit 1
            ;;
    esac
}

get_latest_tag() {
    local tag
    # Follow the /releases/latest redirect instead of the API. The API endpoint
    # is rate-limited to 60 req/hr per IP for unauthenticated clients, which
    # breaks installs on shared/NAT'd networks; the redirect is not limited.
    tag="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
        "https://github.com/$REPO/releases/latest" 2>/dev/null \
        | sed -E 's#.*/tag/##')"
    if [[ -z "$tag" ]]; then
        echo "Error: could not determine latest version from GitHub" >&2
        exit 1
    fi
    echo "$tag"
}

# fetch SRC DEST
#
# Download SRC to DEST. When SRC is a local path (absolute, relative, or a
# file:// URL), copy it directly instead of using curl. This lets the installer
# run against a local mirror / offline directory, and makes it testable without
# network access.
fetch() {
    local src="$1" dst="$2"
    if [[ "$src" == file://* ]]; then
        cp "${src#file://}" "$dst"
    elif [[ "$src" == /* || "$src" == ./* || "$src" == ../* ]]; then
        cp "$src" "$dst"
    else
        curl -fsSL --retry 3 --retry-delay 2 -o "$dst" "$src"
    fi
}

main() {
    local triple version_tag
    triple="$(detect_triple)"

    if [[ -z "$VERSION" ]]; then
        version_tag="$(get_latest_tag)"
    else
        version_tag="$VERSION"
        if [[ "$version_tag" != v* ]]; then
            version_tag="v$version_tag"
        fi
    fi

    local version_no_v="${version_tag#v}"
    local artifact="pifile-${version_no_v}-${triple}.tar.gz"

    echo "==> Platform:  $triple"
    echo "==> Version:   $version_tag"
    echo "==> Artifact:  $artifact"

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    # tmp_dir is local to main(); the EXIT trap runs after main returns, so
    # reference it with ${tmp_dir:-} to avoid an "unbound variable" error
    # under `set -u` when the trap fires post-return.
    trap 'rm -rf "${tmp_dir:-}"' EXIT

    echo "==> Downloading $artifact ..."
    if ! fetch "$RELEASE_BASE_URL/$version_tag/$artifact" "$tmp_dir/$artifact"; then
        echo "Error: failed to download $artifact from $RELEASE_BASE_URL/$version_tag" >&2
        echo "Check that the release exists and the artifact name is correct." >&2
        exit 1
    fi

    # Fail closed on integrity: the checksum file is required. If it's
    # missing, refuse to install rather than shipping an unverified tarball.
    if ! fetch "$RELEASE_BASE_URL/$version_tag/checksums.txt" "$tmp_dir/checksums.txt" 2>/dev/null; then
        echo "Error: failed to download checksums.txt from $RELEASE_BASE_URL/$version_tag" >&2
        echo "Refusing to install without a checksum. Verify the release is complete." >&2
        exit 1
    fi

    # Fail closed on authenticity: the checksums.txt signature is required and
    # must verify against the pinned Ed25519 public key. This proves the
    # checksums (and therefore the tarball) were produced by the pifile
    # maintainers, not tampered with in transit or on the release host.
    if ! fetch "$RELEASE_BASE_URL/$version_tag/checksums.txt.sig" "$tmp_dir/checksums.txt.sig" 2>/dev/null; then
        echo "Error: failed to download checksums.txt.sig from $RELEASE_BASE_URL/$version_tag" >&2
        echo "Refusing to install without a release signature." >&2
        exit 1
    fi

    if ! command -v openssl &>/dev/null; then
        echo "Error: openssl is required to verify the release signature" >&2
        exit 1
    fi

    local pubkey_file
    pubkey_file="$tmp_dir/pifile-signing-key.pub"
    printf '%s\n' "$SIGNING_PUBLIC_KEY" > "$pubkey_file"

    if ! openssl pkeyutl -verify -rawin -in "$tmp_dir/checksums.txt" \
        -sigfile "$tmp_dir/checksums.txt.sig" \
        -pubin -inkey "$pubkey_file" >/dev/null 2>&1; then
        echo "Error: release signature verification FAILED for checksums.txt" >&2
        echo "Refusing to install: the release could not be authenticated." >&2
        exit 1
    fi
    echo "==> Signature OK"

    local expected
    # Anchor to the exact artifact name (end of line) so the entry can't be
    # shadowed by a similarly named archive entry in checksums.txt.
    expected="$(grep -E "^[0-9a-f]{64}  ${artifact}$" "$tmp_dir/checksums.txt" | awk '{print $1}')"
    if [[ -z "$expected" ]]; then
        echo "Error: no checksum entry found for $artifact in checksums.txt" >&2
        echo "Refusing to install an unverified tarball." >&2
        exit 1
    fi

    local actual
    if command -v sha256sum &>/dev/null; then
        actual="$(sha256sum "$tmp_dir/$artifact" | awk '{print $1}')"
    elif command -v shasum &>/dev/null; then
        actual="$(shasum -a 256 "$tmp_dir/$artifact" | awk '{print $1}')"
    else
        echo "Error: neither sha256sum nor shasum is available to verify the download" >&2
        exit 1
    fi

    if [[ "$actual" != "$expected" ]]; then
        echo "Error: checksum mismatch!" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        exit 1
    fi
    echo "==> Checksum OK"

    echo "==> Extracting and running the bundled install.sh ..."
    local extract_dir
    extract_dir="$(mktemp -d)"
    trap 'rm -rf "${tmp_dir:-}" "${extract_dir:-}"' EXIT
    tar -xzf "$tmp_dir/$artifact" -C "$extract_dir"

    if [[ ! -x "$extract_dir/pifile-${version_no_v}-${triple}/install.sh" ]]; then
        echo "Error: tarball does not contain the bundled install.sh" >&2
        exit 1
    fi

    local prefix_args=()
    if [[ -n "$PREFIX" ]]; then
        prefix_args=("PREFIX=$PREFIX")
    fi

    bash "$extract_dir/pifile-${version_no_v}-${triple}/install.sh" "${prefix_args[@]}"
}

main