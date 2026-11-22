## ADDED Requirements

### Requirement: Staging, unstaging and discarding a change
In the diff window `C-c s` (`diff-stage`), `C-c u` (`diff-unstage`) and `C-c k` (`diff-discard`) SHALL act on the change at point, its new lines or the line its removed rows sit on; with the region active they SHALL act on the region's lines instead: the added lines it covers, a region ending at a line's start leaving that line out, and the removed rows of every change it touches. In the list window the same keys SHALL act on files as `s`, `u` and `k` do. Staging SHALL be refused in a `Staged` diff echoing `Already staged`, and unstaging in a `Changes` diff echoing `Not staged`; discarding in a `Staged` diff SHALL unstage. Staging SHALL put the chosen lines in the index and nothing else, and unstaging SHALL take them out of it, the working tree left as it is. A file added, deleted, untracked, renamed or copied on the diff's sides SHALL be staged or unstaged whole. Staging while the file's buffer has unsaved edits SHALL be refused, echoing `Save <path> first`, with git not run. Discarding in a `Changes` diff SHALL edit the file's buffer and not the disk: the chosen added lines SHALL go and the touched changes' removed lines SHALL come back, as one undo group, without a question, the buffer left unsaved. Point away from any change with no region SHALL echo `No change here`. The region SHALL be deactivated after the action. When git succeeds the list SHALL be refreshed; when it fails its message SHALL be echoed.

#### Scenario: Staging one change of several
- **GIVEN** a `Changes` diff with two changes
- **WHEN** `C-c s` is pressed on the first
- **THEN** the patch applied to the index holds the first change alone

#### Scenario: A region inside a change
- **GIVEN** a change adding three lines in place of one
- **WHEN** a region over the second added line is staged
- **THEN** the index takes the removed line out and that added line in, and not the other two

#### Scenario: Unstaging a change
- **WHEN** `C-c u` is pressed on a change in a `Staged` diff
- **THEN** the patch applied to the index puts that change back to `HEAD`'s text

#### Scenario: Discarding edits the buffer
- **WHEN** `C-c k` is pressed on a change in a `Changes` diff
- **THEN** the buffer holds the old side's text there, is unsaved, nothing runs git, and `C-/` brings the change back

#### Scenario: Unsaved edits refuse staging
- **GIVEN** the file's buffer has unsaved edits
- **WHEN** `C-c s` is pressed
- **THEN** `Save <path> first` is echoed and git is not run

#### Scenario: The wrong action for the diff
- **WHEN** `C-c s` is pressed in a `Staged` diff
- **THEN** `Already staged` is echoed

#### Scenario: An untracked file stages whole
- **WHEN** `C-c s` is pressed in an untracked file's diff
- **THEN** the file is staged as `s` in the list stages it

### Requirement: Point in the diff kept across a refresh
When the list is refreshed and point's row is still listed, point in the diff window SHALL stay on its line and column, clamped to the new text, instead of going to the first change.

#### Scenario: Staging keeps point near the change
- **GIVEN** point on the second of three changes of a `Changes` diff
- **WHEN** it is staged and the list refreshes with the file still changed
- **THEN** point is on the same line, not on the first change
