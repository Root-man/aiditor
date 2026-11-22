## MODIFIED Requirements

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

## ADDED Requirements

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
