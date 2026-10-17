# Proposal

## Why

The global bindings are hardcoded in `Core::default`, so the user can't change a key without rebuilding. The spec says keymaps live in TOML files reloadable at runtime (issue #24).

## What Changes

- A bundled `keymap.toml` holds the default bindings, one TOML table per keymap name (`[global]`, or a mode name) mapping `kbd` sequences to command names. It replaces the hardcoded global bindings.
- The user's `keymap.toml` in the config dir, beside `settings.toml`, is bound over the bundled one, so a user entry replaces the default for that sequence and every other default stays.
- Bad entries (a key sequence that doesn't parse, an unknown command, a value that isn't a string, a section that isn't a table) are skipped and reported on the message line; the rest still loads.
- A user file that can't be read or isn't valid TOML is reported and the keymaps in effect stay.
- `C-x C-r` (`reload-settings`) reloads the keymaps too, rebuilt from the bundled file plus the user's.
- Out of scope: unbinding a default key, binding commands registered after load.

## Capabilities

### New Capabilities

### Modified Capabilities
- `keymaps`: bindings come from bundled and user TOML files, with bad entries reported and reload at runtime.

## Impact

`aiditor-core`: `keymap.rs` (parse a keymap file), `lib.rs` (default keymaps from the file, a `Msg::KeymapLoaded`, keymaps keyed by owned names), `harness.rs` (reads `keymap.toml` on reload), new `keymap.toml`. `aiditor`: `main.rs` reads `keymap.toml` from the config dir at startup and on reload. `docs/editor.md` records the decision.
