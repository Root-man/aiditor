# Spec Delta

## MODIFIED Requirements

### Requirement: Changing the buffer deactivates the mark
A command that changes the text of the current buffer SHALL deactivate the mark, leaving it set. Lines logged to `*Messages*` SHALL NOT count as a change to it.

#### Scenario: Typing after selecting
- **WHEN** `C-SPC C-f` is pressed at 0 in `hello` and then `x` is typed
- **THEN** the mark is at 0 and inactive and no region is shown

#### Scenario: Selecting in *Messages*
- **WHEN** `C-SPC C-f` is pressed in `*Messages*`, which logs `Mark set` to it
- **THEN** the mark is active and the region is shown
