# Tasks

## 1. Setting

- [x] 1.1 Add the `hl_line` boolean to `Settings` and `settings.toml`; verify `bundled_defaults_parse` and `bad_files_are_rejected` cover it

## 2. Highlight

- [x] 2.1 Add `display::highlight_line`, which decorates every row of the cursor's line, and call it from `Core::frame` for the selected window or the minibuffer's origin window; verify with frame tests for two windows, wrapping, copy mode, other buffers, the setting and a preview
- [x] 2.2 Verify with a display test that the region and a property's background keep their span backgrounds on a decorated line
- [x] 2.3 Update the frame snapshots and review each diff shows only the new decoration
- [x] 2.4 Describe the behavior in `docs/editor.md` next to line numbers

## Workflow follow-up

- Archive the change after the review.
