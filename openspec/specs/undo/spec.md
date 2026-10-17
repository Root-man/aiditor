# undo Specification

## Purpose
Per-buffer undo and redo: edits are recorded in groups, one per command, so a command's whole effect can be reverted and re-applied with point restored.

## Requirements

### Requirement: Undo reverts the last command's edits
Each buffer SHALL keep its own undo history. Undo SHALL revert every edit of the most recent undo group and put point where it was before the command that made that group. Undoing repeatedly SHALL revert older groups in turn.

#### Scenario: Undo a deletion
- **WHEN** `abc` is typed, `<left>` is pressed, then `DEL`, then undo
- **THEN** the text is `abc` and point is 2

#### Scenario: Undo everything
- **WHEN** `ab RET c` is typed into an empty buffer and undo is pressed until nothing is left
- **THEN** the text is empty and point is 0

### Requirement: Redo re-applies what undo reverted
Undo SHALL push the group it reverts onto a redo stack. Redo SHALL re-apply the most recently undone group and put point where it was after that command. Any new edit SHALL clear the redo stack.

#### Scenario: Round trip
- **WHEN** `ab RET c` is typed, undo is pressed three times and redo three times
- **THEN** the text is `ab\nc` and point is 4

#### Scenario: A new edit clears redo
- **WHEN** `ab` is typed, undo is pressed, `x` is typed and redo is pressed
- **THEN** the text is `x` and the message line reports that there is nothing to redo

### Requirement: Each command is one undo group
Edits made by one command SHALL form one group. Commands that edit nothing SHALL make no group. Consecutive typed characters SHALL amalgamate into one group of at most 20; any other command ends the run.

#### Scenario: Typing amalgamates
- **WHEN** `hello` is typed, then undo
- **THEN** the text is empty

#### Scenario: Twenty characters per group
- **WHEN** 25 characters are typed, then undo
- **THEN** the first 20 characters remain

#### Scenario: Another command breaks the run
- **WHEN** `ab`, `<left>`, `<right>` and `cd` are typed, then undo
- **THEN** the text is `ab`

### Requirement: Undo and redo keys
`C-/`, `C-_` and `C-x u` SHALL undo; `C-?` SHALL redo. With nothing to undo or redo, the key SHALL change nothing and the message line SHALL say there is no further undo or redo information.

#### Scenario: Nothing to undo
- **WHEN** undo is pressed in a new buffer
- **THEN** the text and point do not change and the message line says there is no further undo information
