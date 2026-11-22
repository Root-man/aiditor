# diff-view Specification

## Purpose
The diff view: a temporary full-frame layout of the current workspace listing the changed files of every root beside a window for their diff.

## Requirements

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

### Requirement: The list shows each root's staged and unstaged files
The list SHALL ask git status for every root of the workspace's project, in the project's order, and show `Reading status...` until the answer. Each git root SHALL be a bold heading with its directory's name and `⎇ <branch>` in the branch face, after a fold mark (`▾` open, `▸` folded) when it has files, followed by a `Staged` group (files with a staged change) and a `Changes` group (files with an unstaged change, untracked included), each heading in the group face with its count, and each file a row with its status letter in the change's face and its path relative to the root, its directory part dim. A file staged and changed again SHALL be listed in both groups. A group without files SHALL be left out; a root with no changes SHALL say `No changes`, a root outside a repository `not a git repository`, and a git failure its error on one line. Roots SHALL be separated by a blank line. The list SHALL never wrap: a file row wider than its window SHALL have its path cut from the left, starting with `…`. The row of point SHALL be in `hl-line` whether or not the list's window is selected.

#### Scenario: Two roots on different branches
- **GIVEN** a project whose two roots are repositories on branches `main` and `topic`, each with a changed file
- **WHEN** the answer arrives
- **THEN** each file is listed under its own root's heading, which names that root's branch

#### Scenario: Staged and changed again
- **WHEN** a file's change is staged and it is changed again
- **THEN** it is listed under both `Staged` and `Changes`

#### Scenario: Clean and non-git roots
- **WHEN** one root has no changes and another is not a repository
- **THEN** the first says `No changes` and the second `not a git repository`, and neither has a fold mark

#### Scenario: A long path
- **WHEN** a file's row is wider than the list's window
- **THEN** its path starts with `…` and ends with the file's name, and the row fills the window

#### Scenario: The shown file stays marked
- **WHEN** the diff window is selected
- **THEN** the shown file's row in the list is in `hl-line`

### Requirement: The list changes only on refresh
The list's rows SHALL change only when refreshed with `g` in the list or `C-c g` in either window, or when `TAB` folds or unfolds a root; changes on disk SHALL NOT change it. A refresh SHALL keep point on the row of the file (or heading) it was on when that row is still listed, else on the first file. Motion, search and copying SHALL work as in any read-only buffer.

#### Scenario: Refresh keeps point on its file
- **GIVEN** point on a file's row and a new file changed that sorts before it
- **WHEN** the list is refreshed
- **THEN** point is on the same file's row, one line lower

#### Scenario: No refresh, no change
- **WHEN** a file changes on disk and the list is not refreshed
- **THEN** the list's text is unchanged

### Requirement: One mode line under both windows
While the list's window has a window to its right along its bottom edge, the list's window SHALL draw no mode line, and that window's mode line SHALL span from the list's left edge to its own right edge, drawn active while either window is selected.

#### Scenario: One mode line
- **WHEN** the view is open
- **THEN** the frame draws one mode line, starting at the frame's left edge, for the right window's buffer

### Requirement: The file at point is shown as an inline diff
While the view is open, when point in the list rests on a file's row, the right window SHALL show that file's diff. For a file under `Changes` the old side SHALL be the index and the new side the file's own buffer, visited if it isn't open, editable and saved as anywhere else. For a file under `Staged` the old side SHALL be `HEAD` and the new side a read-only buffer holding the index text. The new side's lines SHALL be the buffer's; each run of removed lines SHALL be drawn as rows above the line that replaced them (after the last line when they ended the file), on the removed face's background with no line number. Added and changed lines SHALL be drawn on the added face's background. The gutter SHALL number the new side, the index text's buffer included, and SHALL have a sign column between the numbers and the text: `+` in the added face on an added line's first row, `-` in the removed face on a removed row's first row, blank elsewhere. When a file's diff first arrives, point in the diff window SHALL go to the start of its first change, its line centred only if it isn't shown; later re-diffs SHALL leave point where it is. Moving point onto another file SHALL show that file's diff; a row that is not a file SHALL leave the shown diff. An untracked file SHALL show as all added and a deleted one as all removed. A binary, non-UTF-8 or unreadable file SHALL show a one-line reason instead.

#### Scenario: A changed file
- **GIVEN** a file whose index text is `a b c` (one per line) and whose working tree is `a B c d`
- **WHEN** point moves onto it under `Changes`
- **THEN** the right window shows the file's buffer with a removed row `b` signed `-` and unnumbered above `B`, and `B` and `d` signed `+` on the added background

#### Scenario: Staged and unstaged are two diffs
- **GIVEN** a file staged and changed again
- **WHEN** point moves from its `Staged` row to its `Changes` row
- **THEN** the first shows `HEAD` → index over a read-only buffer and the second index → working tree over the file's buffer

#### Scenario: A binary file
- **WHEN** point moves onto a binary file
- **THEN** the right window shows why it has no diff, on one line

#### Scenario: Opening on the first change
- **GIVEN** a file whose first change is on line 40
- **WHEN** its diff is shown
- **THEN** point in the diff window is on line 40, and typing elsewhere afterwards does not move it back

#### Scenario: Point on an added line
- **WHEN** point is on an added line
- **THEN** the line keeps the added background over `hl-line`

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
The mode line under the view, while the diff window shows the shown diff's buffer, SHALL show what the buffer's mode line shows (the file, its root when the project has several, the modified mark), then the added and removed line counts as `+<added> −<removed>` and the sides compared (`HEAD → index` or `index → working tree`); on its right, after the branch (named for a `Staged` diff's index text too, which has no file) and before point's position, `change <i>/<n>` when point is in a change, else `<n> changes` (`1 change` for one).

#### Scenario: Counts and position
- **GIVEN** a diff adding 2 lines and removing 1 in two changes, point in the second
- **THEN** the mode line shows `+2 −1`, `index → working tree` and `change 2/2`, the last just before point's line and column

### Requirement: The diff is highlighted as the file is
The diff view SHALL highlight code as the file's own buffer would. Unchanged and added lines SHALL carry the faces the new side's buffer gives them; the read-only buffer holding a `Staged` diff's index text SHALL take the file's grammar. Removed rows SHALL take their faces from a parse of the whole old text with the file's grammar, so a removed line inside a multi-line string or comment is coloured as one; old text over the size limit SHALL be parsed off the core thread, its rows plain until the parse arrives. Rows SHALL follow a theme change. A file in a language without a grammar SHALL show plain text on the diff backgrounds.

#### Scenario: Old and new tokenise differently
- **GIVEN** a Rust file whose old text opens a block comment that the new text deleted
- **WHEN** its diff is shown
- **THEN** a removed line from inside the old comment has the comment face, and the same text on the new side has code faces

#### Scenario: A staged diff
- **WHEN** a Rust file's `Staged` diff is shown
- **THEN** its index buffer's keywords have the keyword face

#### Scenario: A large old side
- **GIVEN** old text over the size limit
- **WHEN** its diff is shown
- **THEN** its removed rows are plain until the parse arrives, then coloured

#### Scenario: No grammar
- **WHEN** the diff of a file with no known language is shown
- **THEN** removed rows are plain text on the removed background

### Requirement: Folding a root
`TAB` in the list SHALL fold the root of the row at point, hiding its groups and files and showing `▸`, or unfold it, and SHALL move point to the root's heading. A refresh SHALL keep the folded roots folded. A root without files SHALL not fold.

#### Scenario: Fold and refresh
- **GIVEN** point on a file under a root
- **WHEN** `TAB` is pressed and the list is refreshed
- **THEN** the root's heading shows `▸`, its files are hidden, and point is on the heading

### Requirement: Key hints in either window
The echo line SHALL show the view's key hints while the list or the diff window is selected and no message or minibuffer is showing, with the keys bound in that window; two keys sharing a prefix SHALL read as one, `C-c n/p`.

#### Scenario: Hints in the diff window
- **WHEN** the diff window is selected
- **THEN** the echo line shows `C-c n/p` for changes and `C-c q` for quit
