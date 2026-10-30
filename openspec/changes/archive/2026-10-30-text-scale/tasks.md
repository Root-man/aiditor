# Tasks

## 1. Zoom

- [x] 1.1 Add `default_font_size` to `Settings`, set by `parse`, and a clamped `zoom` step; verify with a settings unit test
- [x] 1.2 Register `text-scale-increase`, `text-scale-decrease` and `text-scale-reset`, and keep the offset when settings load; verify with core tests for steps, the range, reset to the file's size and reload while zoomed

## 2. Keys

- [x] 2.1 Bind the keys in `keymap-linux.toml` and `keymap-macos.toml`, and `M--` in `keymap.toml`; verify with a core test pressing the platform's keys
- [x] 2.2 Move test uses of `C--` as `negative-argument` to `M--`; verify `cargo test --workspace` passes
- [x] 2.3 Keep the zoom keys with the editor in char mode on Linux; verify with a terminal test that nothing reaches the shell

## 3. Frontend and docs

- [x] 3.1 Verify through GPUI's test platform that a zoom key relays out the frame at the new cell size and line height
- [x] 3.2 Update `C--` in `docs/editor.md` and record the decision next to the font settings; verify by grep
