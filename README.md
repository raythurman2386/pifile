# Pifile

A keyboard-first file manager built with [GPUI Kit](https://github.com/longbridge/gpui-kit).

It is a directory browser. It does not special-case piwrite, picalc, raven, or hearth. Opening a file uses `xdg-open`. Theme colors come from the active desktop theme when present.

## Install

Netinstaller (recommended) — downloads the latest release, verifies its
Ed25519 signature and SHA-256 checksum (a missing or bad signature refuses the
install), then runs the bundled `install.sh`:

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/pifile/main/scripts/netinstall.sh | sh
# or a specific version:
curl -fsSL https://raw.githubusercontent.com/raythurman2386/pifile/main/scripts/netinstall.sh | sh -s -- v0.1.0
```

User-local install from a checked-out repo (builds from source; binary, icon,
launcher). No root:

```sh
./scripts/install.sh
```

That puts `pifile` on `~/.local/bin`, a desktop entry in the app launcher, and registers it for `inode/directory`. Then:

```sh
pifile
pifile ~/Projects
```

Uninstall with `./scripts/uninstall.sh`.

Tagged releases (`v*`) build Linux x86_64 and aarch64 tarballs on GitHub Actions
and publish them with a `checksums.txt`. Unpack the tarball for your
architecture and run `./install.sh` inside.

The prebuilt binaries target glibc 2.39+ (Ubuntu 24.04, Debian 12/13, Raspberry
Pi OS). Older distros should build from source.

## Run from source

Needs the toolchain pinned in `rust-toolchain.toml` (1.97).

```sh
cargo run --release
cargo run --release -- ~/Projects
```

On a Raspberry Pi 500+ / Pi 5 (or any arm64 Linux host):

```sh
cargo build --release
```

## Theme lookup

First existing `colors.toml` or `theme.conf` wins:

1. `$PIFILE_THEME_DIR`
2. `~/.local/state/omarchy/current/theme`
3. `~/.config/omarchy/current/theme`
4. `~/.local/state/pimarchy/current/theme`
5. `~/.config/pimarchy/current/theme`
6. built-in Omarchy fallback (`#101010` / `#eeeeee` / `#5584aa`)

The whole window follows one tonal family: the file area paints with
`background`/`foreground`, panels (sidebar, toolbar, status bar) with
`lighter_background`, and secondary text with `muted`. Themes without those
keys get them derived from `background`/`foreground`. Selected rows and the
active place use a translucent `accent` wash, and `selection` (often a light
text-highlight color) is never used as a panel background.

## Shortcuts

| Action | Key |
|---|---|
| Parent | `Backspace` / `Alt+Up` |
| Open | `Enter` |
| Open with system | `Ctrl+Enter` |
| New folder | `Ctrl+Shift+N` |
| Rename | `F2` |
| Trash | `Delete` |
| Permanent delete | `Shift+Delete` |
| Copy / cut / paste | `Ctrl+C` / `X` / `V` |
| Hidden files | `Ctrl+H` |
| Refresh | `F5` |
| Home | `Alt+Home` |
| Confirm prompt | `Enter` |
| Cancel prompt | `Escape` |
| Quit | `Ctrl+Q` |

## Release signing

Releases are authenticated with Ed25519 signatures over `checksums.txt`, so a
tampered release (or a look-alike from somewhere else) fails verification. The
model keeps the secret key offline forever — it is never committed, never in
CI, never uploaded:

1. `scripts/gen-signing-key.sh` generates the keypair in `~/.pifile/signing`
   (once, on the maintainer's machine). The public key is committed here as
   `pifile-signing-key.pub` and pinned in the installer.
2. After a release publishes, `scripts/sign-releases.sh v<version>` downloads
   its `checksums.txt`, signs it offline with the secret key, and writes
   `checksums.txt.sig` under `~/.pifile/signing/releases/<version>/`.
3. `scripts/upload-release-sigs.sh v<version>` attaches the signature back to
   the release (`gh release upload --clobber`). Signing and uploading stay
   separate scripts so the secret key never touches a network call.

The netinstaller verifies the signature against the pinned public key and
refuses to install if it is missing or bad (fail closed), then checks the
tarball's SHA-256 against the signed `checksums.txt`.

Colors come from `~/.local/state/omarchy/current/theme/colors.toml` when present, then the other paths above.

## Scope

v0: list, navigate, trash, permanent delete, rename, mkdir, copy/cut/paste (rename-across-device falls back to copy), places sidebar, theme colors, `pifile [path]`.

Not yet: dual pane, thumbnails, recursive search, archive UI, directory watcher, first-class editor hooks.
