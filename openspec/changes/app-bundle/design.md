# Design

## Context

There is no packaging today: `cargo run -p aiditor` or the binary from `target/`. GPUI 0.2.2 takes the Cmd-Tab name and icon from the bundle's Info.plist on macOS, and on Linux matches a window to a desktop entry by its Wayland app id or X11 class (`WindowOptions::app_id`).

## Goals / Non-Goals

**Goals:** one script, no new dependencies, stock platform tools only.

**Non-Goals:** notarization, a DMG or package, a `aiditor` command-line shim on macOS, file associations, a real icon, fixing the Dock's minimal environment (the rest of this epic).

## Decisions

- **A shell script, not cargo-bundle.** `cargo-bundle` would be a new tool to install for a dozen lines of `cp`, a heredoc plist, `iconutil` and `codesign`. The script self-checks with `plutil -lint` and `codesign --verify` on macOS and `desktop-file-validate` on Linux when it is installed.
- **Bundle id `io.github.root-man.aiditor`.** Reverse-DNS of the GitHub home, the Flathub convention for projects without a domain.
- **Install to `~/Applications`.** Spotlight indexes it and no admin rights are needed. The bundle is assembled in a temp dir (a copy under `target/` would show up in Spotlight as a second aiditor) and moved in last, so a failed build keeps the earlier install.
- **Version read from the manifest.** The script reads `version` from `crates/aiditor/Cargo.toml` with a grep, which is enough for one crate.
- **Icon committed twice: `aiditor.svg` and a 1024 px `aiditor.png`.** Linux installs the SVG as a scalable icon; macOS builds `aiditor.icns` from the PNG with `sips` and `iconutil` at bundle time. Rendering the SVG at bundle time would need a rasterizer neither platform ships reliably.
- **Linux copies the binary to `~/.local/bin`.** The entry needs a stable absolute path; `target/` is wiped by `cargo clean`. `Exec` uses the absolute path, so the entry works even when `~/.local/bin` is not on the launcher's `PATH`.

## Risks / Trade-offs

- [The placeholder PNG and SVG can drift] → both are placeholders; a real icon replaces both.
- [Ad-hoc signature changes on every build] → macOS may re-ask for permissions (like file access) after a rebuild; acceptable for a local build.
