# Design

## Context

Keymaps are `Keymap` tries of `Binding::Command(&'static str)`, stored in `Core.keymaps` by mode name; `Core::default` binds the global keys in code. Settings already load through `Effect::LoadSettings` → the app reads `settings.toml` → `Msg::SettingsLoaded(Result<Option<String>, String>)`, and `C-x C-r` runs `reload-settings`.

## Goals / Non-Goals

**Goals:** bindings from TOML, partial loading with errors, reload through the existing settings path.

**Non-Goals:** unbinding syntax; binding commands registered after the keymap loaded; the echo area itself (#36) — errors use the existing message line.

## Decisions

- **Separate `keymap.toml`, not a section of `settings.toml`.** Settings reject a bad file whole (`deny_unknown_fields`), keymaps must load partially; separate files keep both rules simple. The bundled file is `crates/aiditor-core/keymap.toml`, `include_str!`'d like `settings.toml`.
- **Bind bundled, then user, rather than merging TOML tables.** `settings::load` merges tables by key string, but `"C-x  C-s"` and `"C-x C-s"` are the same sequence spelled differently; binding in order makes the user entry win however it is spelled, and also gets prefix-over-command replacement from `Keymap::bind`.
- **Command names resolve against the registry at load.** `Binding::Command` keeps its `&'static str` by taking the registry's key (`commands.get_key_value`), so an unknown command is caught at load and reported as a bad entry. Startup loads after all built-in commands exist.
- **Keymap names become `String`.** Mode names now come from a file; `HashMap<String, Keymap>` still looks up by `&str`.
- **Plumbing.** `Effect::LoadSettings` makes the app read both files and send `Msg::SettingsLoaded` and a new `Msg::KeymapLoaded` with the same payload type. On `Ok`, keymaps are rebuilt from bundled + user and replace `Core.keymaps` whole, so removed user bindings revert; on a read or TOML error the old keymaps stay. Bad-entry messages are joined into one `keymap: ...` line; if a settings error is already on the line, the keymap error is appended after it with `; ` so neither hides the other.

## Risks / Trade-offs

- [Reload replaces all keymaps] → keymaps added in code would be lost; today only tests do that and they don't reload.
- [Commands registered after load can't be bound from the file] → none exist outside tests; revisit when plugins or later subsystems register commands.
