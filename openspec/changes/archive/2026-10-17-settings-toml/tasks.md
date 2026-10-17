# Tasks

## 1. Settings module in the core

- [x] 1.1 Add `crates/aiditor-core/settings.toml` (font size 14, default theme colors) and `settings.rs` with `load<T>` (recursive table merge, then deserialize) and `Settings`/`Theme` with `deny_unknown_fields` and the font-size range check; unit tests cover partial override, no user file, wrong type, unknown key, out of range, and that the bundled file parses (`cargo test -p aiditor-core settings`)
- [x] 1.2 Wire it into `Core`: `settings` and `message` fields, `Msg::SettingsLoaded`, `Effect::LoadSettings`, `C-x C-r`, message cleared on each key; the harness carries out `LoadSettings` from a file in its temp dir; harness tests: reload applies a new font size, a bad value keeps the previous one and sets the message, the next key clears it (`cargo test -p aiditor-core`)

## 2. Frontend

- [x] 2.1 Add `directories` to `crates/aiditor`; read the user file at startup and on `Effect::LoadSettings`, sending `Msg::SettingsLoaded`; render font, size, line height and colors from settings and the message on the bottom row; `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] 2.2 Manual check: edit the font size in the user file, press `C-x C-r`, text resizes; set it to `"big"`, press `C-x C-r`, an error line appears and the size is unchanged

## Workflow follow-up

- Verify, commit, archive and ship per the `/implement` skill.
