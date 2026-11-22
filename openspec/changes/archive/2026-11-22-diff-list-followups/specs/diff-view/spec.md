## MODIFIED Requirements

### Requirement: The list shows each root's staged and unstaged files
The list SHALL ask git status for every root of the workspace's project, in the project's order, and show `Reading status...` until the answer. Each git root SHALL be a bold heading with its directory's name and `⎇ <branch>` in the branch face, after a fold mark (`▾` open, `▸` folded) when it has files, followed by a `Staged` group (files with a staged change) and a `Changes` group (files with an unstaged change, untracked included), each heading a fold mark, the name in the group face and its count right-aligned in a column 3 digits wide, so that counts line up, and each file a row with its status letter in the change's face and its path relative to the root, its directory part dim. A file staged and changed again SHALL be listed in both groups. A group without files SHALL be left out; a root with no changes SHALL say `No changes`, a root outside a repository `not a git repository`, and a git failure its error on one line. Roots SHALL be separated by a blank line. The list SHALL never wrap: a file row wider than its window SHALL have its path cut from the left, starting with `…`. The row of point SHALL be in `hl-line` whether or not the list's window is selected.

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

#### Scenario: Counts line up
- **GIVEN** a `Staged` group of 1 file and a `Changes` group of 120 files
- **WHEN** the list is shown
- **THEN** the last digit of both counts is in the same column

### Requirement: The list changes only on refresh
The list's rows SHALL change only when refreshed with `g` in the list or `C-c g` in either window, or when `TAB` folds or unfolds a root or a group; changes on disk SHALL NOT change it. A refresh SHALL keep point on the row of the file (or heading) it was on when that row is still listed, else on the first file. Motion, search and copying SHALL work as in any read-only buffer.

#### Scenario: Refresh keeps point on its file
- **GIVEN** point on a file's row and a new file changed that sorts before it
- **WHEN** the list is refreshed
- **THEN** point is on the same file's row, one line lower

#### Scenario: No refresh, no change
- **WHEN** a file changes on disk and the list is not refreshed
- **THEN** the list's text is unchanged

### Requirement: The diff follows edits of its buffer
The view SHALL hold the old side's text. 50 ms after the last edit or revert of the shown buffer, the diff SHALL be worked out again off the core thread and the rows and marks redrawn; an answer for an earlier text SHALL be dropped. Typing in a `Changes` diff SHALL edit the file's buffer, and a `Staged` diff SHALL refuse edits as read-only. Quitting the view SHALL leave the file's buffer open with its edits, modified if unsaved. A file changed on disk while its unmodified buffer is shown SHALL revert as any buffer does, and the diff SHALL follow.

#### Scenario: Typing
- **WHEN** a line is typed into a `Changes` diff and 50 ms pass
- **THEN** the new line is marked added and the rows sit where the diff puts them

#### Scenario: A stale answer
- **WHEN** an answer arrives for a text that has since been edited again
- **THEN** it is dropped and the next answer is drawn

### Requirement: Folding a root
`TAB` in the list on a root's heading SHALL fold that root, hiding its groups and files and showing `▸`, or unfold it, with point staying on the heading. A refresh SHALL keep the folded roots folded. A root without files SHALL not fold.

#### Scenario: Fold and refresh
- **GIVEN** point on a root's heading
- **WHEN** `TAB` is pressed and the list is refreshed
- **THEN** the root's heading shows `▸`, its groups and files are hidden, and point is on the heading

## ADDED Requirements

### Requirement: Folding a group
`TAB` in the list on a `Staged` or `Changes` heading, or on a file under it, SHALL fold that group, hiding its files and showing `▸` before its name with its count kept, or unfold it, and SHALL move point to the group's heading. The other group SHALL stay as it was. A refresh SHALL keep folded groups folded; a folded group whose root is folded SHALL be folded again when the root unfolds.

#### Scenario: Fold one group
- **GIVEN** a root with `Staged` and `Changes` files and point on a file under `Changes`
- **WHEN** `TAB` is pressed
- **THEN** `Changes` shows `▸` and its count, its files are hidden, `Staged` and its files are still shown, and point is on the `Changes` heading

#### Scenario: Group fold survives a refresh
- **GIVEN** a folded `Changes` group
- **WHEN** the list is refreshed
- **THEN** `Changes` is still folded
