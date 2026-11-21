## MODIFIED Requirements

### Requirement: The view replaces and restores the workspace's layout
`diff-status` SHALL save the current workspace's window layout and replace it with two side-by-side windows: the workspace's changed-file list on the left, about a third of the frame wide and selected, and a window on the right showing the buffer that was current until a file's diff is shown there. Quitting the view (`q` in the list, `C-c q` in either window) SHALL restore the saved layout as it was and kill the list; no other buffer SHALL be killed but the view's own buffers holding an index text or why a file has no diff. Running `diff-status` while the view is open SHALL select the list's window. The saved layout SHALL travel with the workspace when another one is shown, and killing a buffer SHALL update the saved layout's windows as it updates the shown ones. The command SHALL refuse in the minibuffer and in the lobby.

#### Scenario: Whatever the layout was
- **GIVEN** a workspace split into three windows
- **WHEN** `diff-status` runs
- **THEN** the frame shows two windows, the list left and selected, the previously current buffer right

#### Scenario: Quitting restores the layout
- **WHEN** the view is quit from either window
- **THEN** the three windows come back with their buffers and the selected window, the list buffer is gone and every other buffer stays open

#### Scenario: Running it again selects the list
- **GIVEN** the view is open with the right window selected
- **WHEN** `diff-status` runs again
- **THEN** the list's window is selected and the layout is unchanged

## ADDED Requirements

### Requirement: The file at point is shown as an inline diff
While the view is open, when point in the list rests on a file's row, the right window SHALL show that file's diff. For a file under `Changes` the old side SHALL be the index and the new side the file's own buffer, visited if it isn't open, editable and saved as anywhere else. For a file under `Staged` the old side SHALL be `HEAD` and the new side a read-only buffer holding the index text. The new side's lines SHALL be the buffer's; each run of removed lines SHALL be drawn as rows above the line that replaced them (after the last line when they ended the file), in the removed face, each labelled with its old line number; added and changed lines SHALL be marked in the added face. The gutter SHALL number the new side. Moving point onto another file SHALL show that file's diff; a row that is not a file SHALL leave the shown diff. An untracked file SHALL show as all added and a deleted one as all removed. A binary, non-UTF-8 or unreadable file SHALL show a one-line reason instead.

#### Scenario: A changed file
- **GIVEN** a file whose index text is `a b c` (one per line) and whose working tree is `a B c d`
- **WHEN** point moves onto it under `Changes`
- **THEN** the right window shows the file's buffer with a removed row `b` labelled 2 above `B`, and `B` and `d` marked added

#### Scenario: Staged and unstaged are two diffs
- **GIVEN** a file staged and changed again
- **WHEN** point moves from its `Staged` row to its `Changes` row
- **THEN** the first shows `HEAD` → index over a read-only buffer and the second index → working tree over the file's buffer

#### Scenario: A binary file
- **WHEN** point moves onto a binary file
- **THEN** the right window shows why it has no diff, on one line

### Requirement: The diff follows edits of its buffer
The view SHALL hold the old side's text. A moment after each edit or revert of the shown buffer, the diff SHALL be worked out again off the core thread and the rows and marks redrawn; an answer for an earlier text SHALL be dropped. Typing in a `Changes` diff SHALL edit the file's buffer, and a `Staged` diff SHALL refuse edits as read-only. Quitting the view SHALL leave the file's buffer open with its edits, modified if unsaved. A file changed on disk while its unmodified buffer is shown SHALL revert as any buffer does, and the diff SHALL follow.

#### Scenario: Typing
- **WHEN** a line is typed into a `Changes` diff and the moment passes
- **THEN** the new line is marked added and the rows sit where the diff puts them

#### Scenario: A stale answer
- **WHEN** an answer arrives for a text that has since been edited again
- **THEN** it is dropped and the next answer is drawn

### Requirement: Moving between changes and files
`C-c n` and `C-c p` in the diff window, and `n` and `p` in the list, SHALL move point in the diff window to the start of the next or previous change; at the last or first change they SHALL say `No next change` or `No previous change` and not move. `C-c f` and `C-c b` in the diff window, and `f` and `b` in the list, SHALL move the list's point to the next or previous file, showing its diff, and say `No next file` or `No previous file` at the ends.

#### Scenario: Stepping through changes
- **GIVEN** a diff with two changes and point above both
- **WHEN** next change runs three times
- **THEN** point stops at the first change, then the second, then says `No next change`

### Requirement: Copying a change's removed lines
`C-c w` in the diff window, and `w` in the list, SHALL put the removed lines of the change point is in on the kill ring, each ending in a newline; a change point is in includes the line its removed rows sit above. With no removed lines there it SHALL say `No removed lines here`.

#### Scenario: Copying
- **WHEN** point is on a changed line whose old text was `b`
- **THEN** copying puts `b` and a newline on the kill ring

### Requirement: Quitting to the file
`C-g` in either window of the view SHALL quit the view as `C-c q` does and then show the shown diff's file in the restored selected window with point where it was in the diff window; for a `Staged` diff the file is visited and point goes to the same line and column. While the region is active, `C-g` SHALL only deactivate it. In the minibuffer, `C-g` and the view's other keys SHALL keep their minibuffer meaning.

#### Scenario: C-g with unsaved edits
- **GIVEN** an edited, unsaved `Changes` diff with point on line 3
- **WHEN** `C-g` is pressed
- **THEN** the layout is restored, its selected window shows the file's buffer with point on line 3, and the buffer is still modified

### Requirement: The diff's mode line
The mode line under the view, while the diff window shows the shown diff's buffer, SHALL show what the buffer's mode line shows (the file, its root when the project has several, the modified mark), then the added and removed line counts as `+<added> −<removed>`, the sides compared (`HEAD → index` or `index → working tree`), and `change <i>/<n>` when point is in a change, else `<n> changes` (`1 change` for one).

#### Scenario: Counts and position
- **GIVEN** a diff adding 2 lines and removing 1 in two changes, point in the second
- **THEN** the mode line shows `+2 −1`, `index → working tree` and `change 2/2`
