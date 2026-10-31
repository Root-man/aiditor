# Tasks

## 1. Core workspace modeline

- [x] 1.1 Share session activity with the workspace modeline and aggregate by owning workspace; verify harness tests exercise waiting/failed/working priority, background changes and session removal.
- [x] 1.2 Replace current-only names with fitted workspace spans in creation order, current/dim faces, neighbour overflow and hidden dots; verify display tests cover switching, renaming, font-size changes, long names, tiny widths and overflow, and update affected snapshots.

## 2. Frontend and clock

- [x] 2.1 Add local HH:MM clock reads and next-minute delay using the installed time library; verify deterministic tests cover fractional-second minute alignment and midnight.
- [x] 2.2 Render workspace spans and a right-inset clock with a retained GPUI minute task; verify frontend tests cover layout, workspace changes and an idle timer wakeup.

## 3. Integration

- [x] 3.1 Update docs/editor.md to describe the completed modeline and defaults, and verify OpenSpec validation passes.
- [x] 3.2 Run formatting, workspace clippy/tests and remaining CI checks, and record the manual frame/key checks or exact steps requiring user verification.
