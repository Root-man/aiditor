# Proposal

## Why

Terminal buffers ([#224](https://github.com/Root-man/aiditor/issues/224)) hold a live grid, but their windows draw an empty line: nothing of the shell is visible. Issue [#225](https://github.com/Root-man/aiditor/issues/225) draws the grid, so `vterm` becomes usable to look at, and full-screen programs like `htop` and `vim` show as they would in any xterm.

## What Changes

- A window showing a terminal buffer displays the grid's live screen (not the scrollback, which is copy mode's, #88): each row as runs of cells with resolved colours and attributes, and the cursor.
- Colours: the 16 ANSI colours come from a new `theme.ansi` palette (VS Code Dark+ defaults); colours a program sets with OSC 4 win; the 256-colour cube, the gray ramp and truecolor show their exact colour; default foreground and background are the theme's. Inverse swaps foreground and background; dim draws the foreground at 2/3 brightness.
- Bold, italic and underline cells are drawn in bold, italic and underlined font runs.
- The cursor is a block, beam or underline as the program sets it with DECSCUSR, absent while the program hides it, two cells wide on a wide char, and drawn hollow in a window that isn't selected.
- Each cell is drawn at its column × the cell width, so wide chars and glyphs from fallback fonts stay on the grid.
- Not in scope: viewing scrollback (#88), typing into the terminal (char mode), strikethrough, undercurl and other underline styles beyond a plain underline, concealed text.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-buffers`: a terminal buffer's window displays its screen's styled cells and cursor.
- `frontend-rendering`: terminal windows are drawn cell by cell on the grid, with the cursor's shape.
- `settings`: the theme gains the 16-colour ANSI palette.

## Impact

- `aiditor-core`: `display.rs` (terminal display types on `WindowDisplay`), `terminal.rs` (screen cells and cursor from the alacritty grid), `frame.rs` (terminal windows build it), `settings.rs` and `settings.toml` (`theme.ansi`). Display-model snapshots gain the new field.
- `aiditor` frontend: `element.rs` paints terminal windows.
- `docs/editor.md`: the output-floods note records what was built.
- No new dependencies.
