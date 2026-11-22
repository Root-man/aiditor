# Spec Delta

## MODIFIED Requirements

### Requirement: The list changes only on refresh
The list's rows SHALL change only when refreshed with `g` in the list or `C-c g` in either window, when `TAB` folds or unfolds a root or a group, or when a stage, unstage or discard from the list succeeds, which refreshes it; changes on disk SHALL NOT change it. A refresh SHALL keep point on the row of the file (or heading) it was on when that row is still listed, else on the nearest file that was listed after it and still is, else the nearest before it, else the first file. A refresh SHALL ask the shown diff again. Motion, search and copying SHALL work as in any read-only buffer.

#### Scenario: Refresh keeps point on its file
- **GIVEN** point on a file's row and a new file changed that sorts before it
- **WHEN** the list is refreshed
- **THEN** point is on the same file's row, one line lower

#### Scenario: No refresh, no change
- **WHEN** a file changes on disk and the list is not refreshed
- **THEN** the list's text is unchanged

#### Scenario: A row that left
- **GIVEN** point on the first of two files under `Changes`
- **WHEN** the list is refreshed and that file is no longer under `Changes`
- **THEN** point is on the second file's row

## ADDED Requirements

### Requirement: Staging, unstaging and discarding from the list
In the list, `s` (`diff-stage`) SHALL stage, `u` (`diff-unstage`) SHALL unstage and `k` (`diff-discard`) SHALL discard what point is on: a file's row acts on that file in its group, a `Staged` or `Changes` heading on every file under it, and a root heading on every file of the root. Staging SHALL take the `Changes` files, unstaging the `Staged` files and their renames' old paths, and discarding SHALL restore each `Changes` file to the index's text, delete each untracked one, and unstage each `Staged` file, restoring before unstaging so a staged change is kept in the working tree. Before discarding any `Changes` file the view SHALL ask, answering `No` or `Yes`, a question naming the file, or the count, whose changes are lost and the untracked files deleted; only `Yes` goes ahead. Staging or discarding a `Changes` file whose buffer has unsaved edits SHALL be refused, echoing `Save <path> first` or `<path> has unsaved edits`, with git not run. An action with nothing to act on SHALL echo `Nothing to stage`, `Nothing to unstage` or `Nothing to discard`. When git succeeds the list SHALL be refreshed; when it fails its message SHALL be echoed and the list left as it was. The list's key hints SHALL include `s/u` for stage/unstage and `k` for discard.

#### Scenario: Staging and unstaging a file
- **GIVEN** point on a modified file under `Changes`
- **WHEN** `s` is pressed and git succeeds
- **THEN** the list is refreshed and the file is listed under `Staged`; `u` on that row returns it to `Changes`

#### Scenario: A heading acts on every file under it
- **WHEN** `s` is pressed on a `Changes` heading with a modified, a deleted and an untracked file
- **THEN** all three are staged in one git run

#### Scenario: A staged rename unstages both paths
- **WHEN** `u` is pressed on a renamed file under `Staged`
- **THEN** git unstages its new and its old path

#### Scenario: Discard asks first
- **WHEN** `k` is pressed on a modified file under `Changes`
- **THEN** the minibuffer asks `Discard changes to <path>? `, and git runs only after `Yes`

#### Scenario: Discarding an untracked file says it is deleted
- **WHEN** `k` is pressed on an untracked file
- **THEN** the question is `Delete untracked <path>? `

#### Scenario: Discarding a staged file unstages it
- **WHEN** `k` is pressed on a file under `Staged`
- **THEN** it is unstaged without a question

#### Scenario: Discarding a root keeps its staged changes
- **GIVEN** a root with a file staged and changed again
- **WHEN** `k` is pressed on the root heading and answered `Yes`
- **THEN** git restores the file and then unstages it, leaving the staged text in the working tree

#### Scenario: Unsaved edits refuse
- **GIVEN** the buffer of a file under `Changes` has unsaved edits
- **WHEN** `s` is pressed on its row
- **THEN** `Save <path> first` is echoed and git is not run

#### Scenario: A failure leaves the list
- **WHEN** git fails with a message
- **THEN** the message is echoed and the list's text is unchanged

#### Scenario: The hints
- **WHEN** the list is selected
- **THEN** the echo line shows `s/u stage/unstage` and `k discard`
