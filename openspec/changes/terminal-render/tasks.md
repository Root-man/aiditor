# Tasks

## 1. Core: terminal display

- [x] 1.1 Add `ansi` (16 colours, VS Code Dark+ defaults) to `Theme` and `settings.toml`; verify a settings test that a user `theme.ansi` overrides the default
- [x] 1.2 Add `display::Terminal`, `Run`, `Cursor` and `CursorShape`, and `terminal: Option<Terminal>` on `WindowDisplay`; accept the updated insta snapshots
- [x] 1.3 Build the screen's runs and cursor in `terminal.rs` (colour resolution, inverse, dim, bold/italic/underline, wide chars alone in their run, blank default cells skipped, DECSCUSR shape, hidden cursor) and fill it in `frame::window_display` for terminal buffers; verify unit tests for every terminal-buffers delta scenario
- [x] 1.4 Verify a harness test that `M-x vterm` output shows in the frame display's terminal rows

## 2. Frontend: paint the grid

- [x] 2.1 Paint terminal windows in `element.rs`: background quads per run, each run shaped with `force_width` at `col × cell` in bold/italic/underline font runs, the cursor as block (char in background colour), beam or underline, hollow outline in an unselected window; verify a GPUI test that `x` after `日本` is painted at the same x as `e` in the row below and the unselected window's cursor is hollow
- [x] 2.2 Record what was built in `docs/editor.md`'s output-floods note (replace "Not yet: drawing the grid")
- [x] 2.3 Run fmt, clippy and tests as CI does, and check `htop` and `vim` by hand on Linux (macOS by the user)
