# Spec Delta

## MODIFIED Requirements

### Requirement: C-g quits
`C-g` after a prefix key SHALL cancel the pending sequence without running a command, and the message line SHALL show `Quit`. `C-g` with no pending prefix SHALL run the `keyboard-quit` command, which deactivates the mark and shows `Quit`.

#### Scenario: Cancel a prefix
- **WHEN** `C-x C-g` is pressed and then `C-s`
- **THEN** nothing is saved, the message line showed `Quit`, and `C-s` is looked up on its own

#### Scenario: C-g on its own
- **WHEN** `C-g` is pressed with no prefix pending
- **THEN** the message line shows `Quit`

#### Scenario: C-g clears the region
- **WHEN** `C-SPC C-f C-f` is pressed in `hello` and then `C-g`
- **THEN** the mark is still at 0, inactive, and the display shows no region
