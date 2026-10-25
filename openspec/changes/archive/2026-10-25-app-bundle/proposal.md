# Proposal

## Why

aiditor runs only as a binary from a shell; it can't be started from the Dock, Spotlight or a Linux launcher, and Cmd-Tab shows no name or icon ([#266](https://github.com/Root-man/aiditor/issues/266)). A bundle id is also what system notifications need later.

## What Changes

- `scripts/bundle.sh` builds the release binary and installs it for the current platform:
  - macOS: assembles `aiditor.app` (Info.plist with bundle id `io.github.root-man.aiditor` and name `aiditor`, a placeholder icon, the binary in `Contents/MacOS`), ad-hoc signs it and copies it to `~/Applications`.
  - Linux: copies the binary to `~/.local/bin/aiditor` and installs `aiditor.desktop` and the icon under `~/.local/share`.
- A placeholder icon (SVG source and a 1024 px PNG) under `crates/aiditor/resources`.
- The window's app id is `aiditor`, so a Linux desktop matches the window to its desktop entry.
- `aiditor .` from a shell still runs the binary directly. No notarization; the bundle is built locally.
- `docs/editor.md` records how to build and install the bundle.

## Capabilities

### New Capabilities
- `app-bundle`: building and installing aiditor as a macOS app bundle or a Linux desktop entry.

### Modified Capabilities

## Impact

- `scripts/bundle.sh`, `crates/aiditor/resources/` (icon, `aiditor.desktop`).
- `crates/aiditor/src/main.rs`: `WindowOptions::app_id`.
- `docs/editor.md`: a bundle paragraph under Subsystems.
