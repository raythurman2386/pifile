# Pifile

A keyboard-first file manager built with [GPUI Kit](https://github.com/longbridge/gpui-kit).

It is a directory browser. It does not special-case piwrite, picalc, raven, or hearth. Opening a file uses `xdg-open`. Theme colors come from the active desktop theme when present.

## Install

User-local install (binary, icon, launcher). No root:

```sh
./scripts/install.sh
```

That puts `pifile` on `~/.local/bin`, a desktop entry in the app launcher, and registers it for `inode/directory`. Then:

```sh
pifile
pifile ~/Projects
```

Uninstall with `./scripts/uninstall.sh`.

Tagged releases (`v*`) build a Linux x86_64 tarball on GitHub Actions. Unpack it and run `./install.sh` inside.

## Run from source

Needs the toolchain pinned in `rust-toolchain.toml` (1.97).

```sh
cargo run --release
cargo run --release -- ~/Projects
```

On a Raspberry Pi 500+ / Pi 5:

```sh
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
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

Colors come from `~/.local/state/omarchy/current/theme/colors.toml` when present, then the other paths above.

## Scope

v0: list, navigate, trash, permanent delete, rename, mkdir, copy/cut/paste (rename-across-device falls back to copy), places sidebar, theme colors, `pifile [path]`.

Not yet: dual pane, thumbnails, recursive search, archive UI, directory watcher, first-class editor hooks.
