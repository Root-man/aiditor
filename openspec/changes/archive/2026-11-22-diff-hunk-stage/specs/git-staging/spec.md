## ADDED Requirements

### Requirement: Applying a patch to the index
A step SHALL also be a unified patch of one file, applied to the index alone with the working tree left as it is; a patch that doesn't apply SHALL fail the step with git's message and leave the index unchanged. A patch SHALL carry the `\ No newline at end of file` marker where a side's last line has no newline.

#### Scenario: One change of several
- **GIVEN** a committed file with two changes in the working tree
- **WHEN** a patch holding the first change is applied
- **THEN** `git diff --cached` shows that change alone and `git diff` the other

#### Scenario: Changes at a file's start and end, without a final newline
- **GIVEN** a committed file without a final newline
- **WHEN** patches changing its first line and its last line are applied
- **THEN** the index holds both changes, its last line still without a newline

#### Scenario: A patch that doesn't apply
- **WHEN** a patch whose context isn't in the index is applied
- **THEN** git's message is answered and the index is unchanged
