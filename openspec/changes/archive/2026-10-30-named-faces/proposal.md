# Proposal

## Why

The theme is a fixed list of colours, too few for the approved design (frame 12 and the table in `docs/editor.md`), with no way to set a face's weight or slant, one theme at a time, and a colour remap on reload that can confuse two names sharing a colour. Every later M5 visual issue needs named faces first (#352).

## What Changes

- **BREAKING** The `[theme]` table is rejected. `theme = "<name>"` picks a `[themes.<name>]` table, merged over the bundled `themes.default`.
- A theme holds `ansi`, `faces` and `syntax`. A face is an inline table of `fg`, `bg`, `bold`, `italic` and `underline`, and an unset attribute takes what is underneath.
- **BREAKING** Syntax entries become faces with no bare-colour form.
- The bundled `themes.default` defines the 41 faces the design names; an unknown face, attribute or theme name rejects the file.
- `dim` becomes `session-thought`, `code_block` becomes `session-block`, and code reads faces by name.
- Session buffers store face names, resolved at each redisplay, so changing `theme` and pressing `C-x C-r` restyles them; the colour remap goes.
- The Gruvbox block in the user's `settings.toml` moves to `[themes.gruvbox-dark]`, and `[themes.gruvbox-light]` is added.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `settings`: themes, faces and their validation replace the fixed colour keys.
- `syntax-highlighting`: syntax entries are faces.
- `session-buffers`: rendering names faces, and theme changes need no remap.
- `display-model`: region and wrap marker read faces.
- `frontend-rendering`: cursor, mode lines and the workspace modeline read faces.

## Impact

`settings.rs` and the bundled `settings.toml`, `buffer.rs`, `display.rs`, `syntax.rs`, `markdown.rs`, `agents.rs`, `terminal.rs`, `lib.rs`, the frontend's `editor.rs` and `element.rs`, benches, `docs/editor.md`, and the user's `settings.toml` outside the repo. No new dependencies.
