## Context

The core is a pure state machine with no I/O: the app carries out `Effect`s and sends results back as `Msg`s (see `Effect::Save` / `Msg::Saved`). There is no echo area (#36), command registry (#20) or keymap trie yet; `C-x` is a hardcoded prefix. The frontend hardcodes font, size and colors.

## Goals / Non-Goals

**Goals:** one generic loader every later TOML file can call; M0 font and theme driven by it; reload without restart.

**Non-Goals:** file watching (auto-reload), per-language or per-project settings, theme files or multiple themes, the real echo area, an `M-x` command name for reload.

## Decisions

- **Generic `settings::load<T: DeserializeOwned>(bundled, user) -> Result<T, String>` in `aiditor-core`.** Parse both as `toml::Table`, merge the user table into the bundled one recursively (tables merge, other values replace), then deserialize `T`. Keymaps and project files reuse it with their own `T`. Alternative: `#[serde(default)]` on every field, which duplicates the defaults in Rust and doesn't let a keymap file override one entry of a table.
- **Bundled defaults are `crates/aiditor-core/settings.toml` via `include_str!`,** so defaults are a TOML file users can copy, and a test checks it parses.
- **`deny_unknown_fields`** turns typos into errors, which the spec requires.
- **Colors are TOML hex integers (`background = 0x1e1e1e`) into `u32`**, matching GPUI's `rgb(u32)`; no color-string parser.
- **`font_family: Option<String>`**, `None` meaning the platform font the frontend already picks per OS; one bundled file serves both platforms.
- **Font size validated after deserializing** (6 to 72), in the same `Result`.
- **I/O through effects.** `C-x C-r` pushes `Effect::LoadSettings`; the app reads `ProjectDirs::from("", "", "aiditor").config_dir()/settings.toml` (`directories`, the crate `docs/editor.md` already picks), maps NotFound to `Ok(None)`, other errors to `Err(String)`, and sends `Msg::SettingsLoaded(Result<Option<String>, String>)`. Startup sends the same message before the first frame. The core applies on success or sets the message on failure.
- **Stand-in echo area:** `Core.message: Option<String>`, set on settings errors, cleared at the start of every `Msg::Key`, drawn by the frontend on a bottom row. #36 replaces it.
- **Line height derived from font size** (`font_size * 1.4`), replacing the `LINE_HEIGHT` constant.

## Risks / Trade-offs

- [`C-x C-r` is `find-file-read-only` in Emacs] → temporary; it moves to a named command and a keymap entry with #20 and #24.
- [Message cleared by any key, even before the user reads it] → same as Emacs; fine for M0.
