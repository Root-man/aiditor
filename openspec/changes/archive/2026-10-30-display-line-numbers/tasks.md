# Tasks

## 1. Core

- [x] 1.1 Add the `line_numbers` setting (`off`, `absolute`, `relative`) to `Settings` and `settings.toml`; verify with settings tests for the bundled value and a rejected value
- [x] 1.2 Add the per-buffer override and `display-line-numbers-mode`; verify with harness tests for toggling, the style kept, `off` overridden and a buffer that takes no numbers
- [x] 1.3 Build `Gutter` on `FrameWindow` in `Core::frame` and take its width out of `cols`; verify with display tests for absolute and relative numbers, the current line's face, `↪` on wrapped rows and at a window starting inside a line, growth at ten lines, wrapping in the reduced columns, a too-narrow window, and update the frame snapshots

## 2. Frontend

- [x] 2.1 Draw the gutter in `FrameElement` and offset text and cursor by its width; verify with a GPUI test that a number ends 12 px before the text and the cursor stays over its char
- [x] 2.2 Check a real window with long wrapped lines by screenshot: numbers and `↪` beside the text; the toggle is covered by the GPUI test in 2.1

## 3. Docs

- [x] 3.1 Record the setting, the command and the defaults in `docs/editor.md`; verify the section names #346

## Workflow follow-up

- Archive the change after review.
