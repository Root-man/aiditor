# diff-view Specification

## Purpose
The diff view: a temporary full-frame layout of the current workspace listing the changed files of every root beside a window for their diff.

## Requirements

### Requirement: The view replaces and restores the workspace's layout
`diff-status` SHALL save the current workspace's window layout and replace it with two side-by-side windows: the workspace's changed-file list on the left, about a third of the frame wide and selected, and a window on the right showing the buffer that was current. Quitting the view (`q` in the list, `C-c q` in either window) SHALL restore the saved layout as it was and kill the list; no other buffer SHALL be killed. Running `diff-status` while the view is open SHALL select the list's window. The saved layout SHALL travel with the workspace when another one is shown, and killing a buffer SHALL update the saved layout's windows as it updates the shown ones. The command SHALL refuse in the minibuffer and in the lobby.

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
The list SHALL ask git status for every root of the workspace's project, in the project's order, and show `Reading status...` until the answer. Each git root SHALL be a bold heading with its directory's name and `⎇ <branch>`, followed by a `Staged` group (files with a staged change) and a `Changes` group (files with an unstaged change, untracked included), each heading with its count, and each file a row with its status letter in the change's face and its path relative to the root. A file staged and changed again SHALL be listed in both groups. A group without files SHALL be left out; a root with no changes SHALL say `No changes`, a root outside a repository `not a git repository`, and a git failure its error on one line. Roots SHALL be separated by a blank line.

#### Scenario: Two roots on different branches
- **GIVEN** a project whose two roots are repositories on branches `main` and `topic`, each with a changed file
- **WHEN** the answer arrives
- **THEN** each file is listed under its own root's heading, which names that root's branch

#### Scenario: Staged and changed again
- **WHEN** a file's change is staged and it is changed again
- **THEN** it is listed under both `Staged` and `Changes`

#### Scenario: Clean and non-git roots
- **WHEN** one root has no changes and another is not in a repository
- **THEN** the first says `No changes` and the second `not a git repository`

### Requirement: The list changes only on refresh
The list SHALL change only when refreshed with `g` in the list or `C-c g` in either window; changes on disk SHALL NOT change it. A refresh SHALL keep point on the row of the file (or heading) it was on when that row is still listed, else on the first file. Motion, search and copying SHALL work as in any read-only buffer.

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
