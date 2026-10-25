# Tasks

## 1. Bundle

- [x] 1.1 Add the placeholder icon (`aiditor.svg`, 1024 px `aiditor.png`) and `aiditor.desktop` under `crates/aiditor/resources`; `desktop-file-validate` accepts the entry where available
- [x] 1.2 Set `WindowOptions::app_id` to `aiditor` in `main.rs`; `cargo build -p aiditor` passes
- [x] 1.3 Write `scripts/bundle.sh`: release build, then the macOS bundle (plist, icns, sign, install in `~/Applications`) or the Linux install; running it on macOS installs a bundle that passes `plutil -lint` and `codesign --verify`
- [x] 1.4 Record how to build and install the bundle in `docs/editor.md`
- [x] 1.5 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
- [x] 1.6 Launch the installed bundle from Spotlight and check its name and icon in Cmd-Tab
