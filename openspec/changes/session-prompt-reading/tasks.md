# Tasks

## 1. Window state

- [x] 1.1 Add `follow` and the two remembered points (`parts`) to `Window`, with `remember`, `remember_at`, `remembered` and `shown`; release the markers with the others

## 2. Display

- [x] 2.1 `session_display`: with point in the prompt area keep the stored top unless following, clamp it to the end and write it back; set `follow` from the rows shown
- [x] 2.2 `agent-send-input` sets `follow`

## 3. Commands

- [x] 3.1 Record point's part in `Ctx::call`'s post-command hook for session buffers
- [x] 3.2 `scroll_lines` and `recenter` act on the transcript rows from the prompt area, moving the reading point into view
- [x] 3.3 `agent-toggle-prompt` and `C-t` in `agent-session-mode`
- [x] 3.4 `self-insert-command` in the transcript inserts at the prompt point

## 4. Tests and docs

- [x] 4.1 Core tests with the docked pane for each acceptance criterion
- [x] 4.2 Update the docked prompt section of `docs/editor.md`
