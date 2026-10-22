# Design

## Context

- Every buffer change in the selected window, other than previews and killed buffers, goes through `Core::switch_to` (`visiting.rs`):
  - `find-file` and `project-find-file`, both straight away and once a read answers (`visit`);
  - `switch-to-buffer` and `switch-to-buffer-other-window`, through `switch_to_name`;
  - `project-grep` into another file, through `find_file`.
- Previews show buffers with `Window::show` on a stand-in window and never call `switch_to`.
- Jumps within the buffer happen in two places:
  - `project_grep`, through `goto_line_col`, when the hit's file is already current;
  - `swiper`, through `Window::jump`.
- Buffers have markers that follow edits and vanish with the buffer (`Buffer::add_marker`, `marker`, `remove_marker`). A window's mark ring already uses them.
- A workspace not current keeps its windows in `Workspace::windows`. The `Workspace` struct lives for the workspace's whole life, current or not.

## Goals / Non-Goals

**Goals:** one recording function called from the three jump sites, and history state that needs no swapping when the workspace changes.

**Non-Goals:** recording at the command loop (diffing buffer and point around each command), since async visits switch buffers outside the command that asked for them.

## Decisions

- **State on `Workspace`**: `back` and `forward` hold `(BufferId, MarkerId)` entries, newest last. They sit on the struct directly rather than in `Windows`, so a workspace switch leaves them alone. The jump code finds the current workspace through the `workspaces` field, so the borrow stays disjoint from `buffers`.
  - Alternative: fields on `Core` swapped in and out like the windows. That means more code for no gain.
- **`Core::push_jump`** empties `forward`, removing its markers, as Emacs's `xref-push-marker-stack` does. Then it adds a `Stay` marker at the selected window's point, unless the newest back entry is that same buffer and position, and drops the oldest entry past 100, removing its marker.
- **Recording sites:**
  - `switch_to` records only when the buffer changes. Its show part becomes `show_buffer` (raise, then `Window::show` if the buffer differs), which going back and forward reuse without recording.
  - `project_grep` notes the selected buffer before `find_file`. When the hit's file is that same buffer, it records before `goto_line_col`. When the buffer changed, `switch_to` has already recorded.
  - `swiper` records when `p != start`, before it jumps, whatever the region's state.
- **Going back and forward** use one function with the two lists in opposite roles:
  1. Pop entries until one is live (the buffer exists and the marker resolves) and differs from the window's buffer and point. Remove each popped marker.
  2. Push the current position onto the other list as a new marker. Forward pushes onto `back` directly, not through `push_jump`, so `forward` is kept.
  3. `show_buffer`, then `Window::jump` to the position. `Window::jump` already centers an off-screen line.
- **Workspace kill** removes the markers of both lists from the buffers still live (`*Messages*` is global), so none linger there.
- **Commands** `xref-go-back` and `xref-go-forward` take no arguments. They are bound to `M-,` and `C-M-,` in `keymap.toml` and live in a new `jump.rs`, next to `mark.rs`.

## Risks / Trade-offs

- [Launch and recovery visits also pass through `switch_to` and record the `*scratch*` position] → harmless: back returns there, as Emacs's global mark ring would.
- [A file read that answers after the user moved records where point is then, not where the find started] → reads take milliseconds, so the user rarely moves first.
- [Up to 200 markers per workspace are walked on every edit to their buffers] → the existing `shortcut:` note on marker walking covers it. The cap keeps it small.
