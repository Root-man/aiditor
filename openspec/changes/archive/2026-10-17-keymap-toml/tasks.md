# Tasks

## 1. Keymap file loading in the core

- [x] 1.1 Add `crates/aiditor-core/keymap.toml` with today's global bindings and a parser in `keymap.rs` that builds keymaps from bundled + user text, skipping and reporting bad entries; unit tests cover a user override, a bad key, an unknown command, a non-string value and a non-table section
- [x] 1.2 Build `Core::default`'s keymaps from the bundled file, key keymaps by `String`, add `Msg::KeymapLoaded`, keep old keymaps on read/TOML errors; existing harness tests still pass
- [x] 1.3 Harness reads `keymap.toml` on `Effect::LoadSettings`; harness tests cover rebind + reload, revert on delete, bad entry among good, invalid TOML keeps keymaps, a user mode table

## 2. App and docs

- [x] 2.1 `main.rs` reads `keymap.toml` from the config dir at startup and on reload; `parse_args` test updated; `cargo test` passes
- [x] 2.2 Record the decision in `docs/editor.md` under Commands and keymaps
