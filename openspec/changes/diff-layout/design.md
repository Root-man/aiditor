# Design

## Context

Workspaces swap whole `Windows` (selected window, the others, the split tree) in and out. `*Sessions*` is the closest list: a read-only buffer kind rebuilt from answers, keeping point on its row. `FileOp::GitStatus` answers each root's repository, branch and files, with no request id.

## Goals / Non-Goals

**Goals:** the layout, the list, refresh, quit and one mode line (#504).

**Non-Goals:** the diff itself, file motion (`n`/`p`/`RET`), staging, folding roots (`TAB`, the `▾` glyph), path truncation with `…`, the shown file's `hl-line` row (all #505 onwards).

## Decisions

- **The saved layout lives in `Windows`** as `saved: Option<Box<Windows>>`, so it moves with the workspace for free; `windows_mut` also yields the saved windows, so killing a buffer, releasing a workspace and the sessions list's point updates reach them. No nesting: `diff-status` while open only selects the list.
- **The list is found by kind** (`Kind::DiffList`) in the current workspace, one per workspace, named `*Diff*<workspace>`; the view is open when `windows.saved` is set.
- **Answers match by roots.** `Msg::GitStatus` carries no request id; every pending list whose roots equal the answer's takes it. Each `g` counts one pending answer, so a second `g` before the first answer rebuilds twice, the later status last.
- **One mode line.** `FrameWindow` gains `bar: Option<(Rect, bool)>`: where its mode line is drawn and whether it is active, `None` for none. The list's window keeps its full height (its last row sits under the bar); the window right of it along its bottom gets a bar from the list's left edge. The frontend paints each bar after the windows' text, clipped to the bar, so it covers the list's bottom.
- **Keys.** The list's major mode `diff-list-mode` binds `g` and `q`; while the view is open a `diff-view` keymap layer (above the major mode) binds `C-c g` and `C-c q` in both windows.
- **Width.** The split ratio is 0.32, frame 15's 460 of 1440 px.

## Risks / Trade-offs

- A layout the user changes inside the view (`C-x 2` in the right window) loses the shared bar until it lines up again; quitting still restores the saved layout.
- Until #505 the right window shows the previous buffer, not a diff.
