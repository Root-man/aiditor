## Why

Every later TOML file (keymaps, projects, the agent registry, formatters, theme) needs the same loading, validation and reload path, and the walking skeleton hardcodes its font and colors. Issue #44 adds that one settings module, with M0's font and default theme as its first users.

## What Changes

- A settings module in `aiditor-core`: bundled defaults compiled in, a user file from the platform config dir merged over them, validated by serde with unknown keys rejected.
- M0 settings: font family (platform default when unset), font size, and a default theme (background, foreground and cursor colors).
- `C-x C-r` reloads the settings. A bad file reports an error and keeps the previous settings.
- A one-line message at the bottom of the frame shows settings errors until the next key. It stands in for the echo area (#36), which replaces it.
- The frontend draws with the settings' font, size and colors instead of constants.

## Capabilities

### New Capabilities
- `settings`: loading TOML settings from bundled defaults and the user's config dir, validation, error reporting and reload.

### Modified Capabilities
None.

## Impact

- `crates/aiditor-core`: new `settings` module and bundled `settings.toml`; `Core` gains settings and a message, `Msg::SettingsLoaded`, `Effect::LoadSettings`, the `C-x C-r` key.
- `crates/aiditor`: reads the user file at startup and on reload; renders from settings; new `directories` dependency (already chosen in `docs/editor.md`).
- The test harness carries out the new effect.
