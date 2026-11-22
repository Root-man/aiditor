# Design

## Context

The view is open while the current `Windows` holds the saved layout (`windows.saved`). Every buffer reaching the selected window goes through `Core::switch_to` or `Core::show_buffer`; other windows get one through `display_buffer`; the tree changes only in `split_window`, `delete_window` and `delete_other_windows`. There are no window resizing commands. The view itself shows buffers with `Window::show` directly (`attach`) and the list with `show_buffer` from `diff-status`.

## Decisions

- **Hook the chokepoints, not each command.** `switch_to` and `show_buffer` quit the view when the buffer differs from the selected window's and isn't the view's list; `display_buffer`, `split_window`, `delete_window` and `delete_other_windows` quit it first (the deletions only outside the minibuffer, so their errors there are unchanged). This catches every command and answer at once, present and future, instead of a list per command. `switch_to` quits before recording the jump, so the jump is of the restored window.
- **Quitting is split out of `diff-quit`** into a `Core` method, used by both, so the layout restore, shown-diff drop and list kill stay in one place.
- **Answers that arrive later quit then.** `C-x C-f` of a file not yet open quits when the file's answer arrives, since that is when it is shown; the visit's window check runs before, against the view's window it was asked in.
- **A buffer killed by quitting is not shown** (the index or reason buffer picked with `C-x b`): `show_buffer` returns when it is gone.
- **Left in place:** `kill-buffer` (already updates the saved layout), an agent rollback replacing a session in every window, minibuffer previews' stand-in window, and workspace switches.

## Risks / Trade-offs

- [A server edit for a non-current workspace that is in the view shows its file in that workspace's view window] → shortcut comment; quitting a non-current workspace's view needs the restore to work on saved windows.
- [`*Workspaces*` refresh keeps points of windows that the quit removed] → skip vanished window ids instead of expecting them.
