# mark Specification

## Purpose
The mark: a second position in the window that, with point, bounds the region, as in Emacs's transient-mark-mode, plus the mark ring that remembers earlier marks.

## Requirements

### Requirement: The mark is active or inactive
A set mark SHALL be either active or inactive. Only an active mark makes the region shown; region commands SHALL still use the mark when it is inactive.

#### Scenario: A pushed mark is inactive
- **WHEN** `M->` is pressed in `abc` with point at 0
- **THEN** the mark is at 0 and inactive

### Requirement: Setting the mark
`set-mark-command` (`C-SPC`) SHALL push the current mark, if any, onto the mark ring, set the mark at point, activate it and show `Mark set`.

#### Scenario: Set and extend
- **WHEN** `C-SPC C-f C-f` is pressed with point at 0 in `hello`
- **THEN** the mark is at 0 and active, point is 2, and the message is `Mark set` after `C-SPC`

#### Scenario: Setting again pushes the old mark
- **WHEN** the mark is at 0 and `C-SPC` is pressed at 3
- **THEN** the mark is at 3 and the mark ring's first entry is 0

### Requirement: Toggling the region
`set-mark-command` run right after `set-mark-command` SHALL leave the mark where it is and toggle whether it is active, showing `Mark deactivated` or `Mark activated`.

#### Scenario: C-SPC C-SPC
- **WHEN** `C-SPC C-SPC` is pressed at 2
- **THEN** the mark is at 2, inactive, and the message is `Mark deactivated`

### Requirement: Popping the mark
`set-mark-command` with a `C-u` prefix SHALL move point to the mark, make the mark ring's first entry the mark, put the old mark at the ring's end and deactivate the mark. With no mark it SHALL fail with `No mark set in this buffer`.

#### Scenario: Back to an earlier mark
- **WHEN** marks are set with `C-SPC` at 1 and then at 3 in `hello`, point moves to 5, and `C-u C-SPC` is pressed
- **THEN** point is 3, the mark is 1 and the ring holds 3

#### Scenario: No mark to pop
- **WHEN** `C-u C-SPC` is pressed and no mark was ever set
- **THEN** point does not move and the message is `No mark set in this buffer`

### Requirement: The mark ring is bounded
The mark ring SHALL hold at most 16 marks; pushing onto a full ring SHALL drop its oldest mark.

#### Scenario: Seventeen pushes
- **WHEN** the mark is set 18 times at different positions
- **THEN** the ring holds 16 marks, the most recent first

### Requirement: Exchanging point and mark
`exchange-point-and-mark` (`C-x C-x`) SHALL swap point and mark and activate the mark. With no mark it SHALL fail with `No mark set in this buffer`.

#### Scenario: Swap
- **WHEN** the mark is at 1, point at 4, the mark is inactive, and `C-x C-x` is pressed
- **THEN** point is 1, the mark is 4 and active

#### Scenario: No mark to swap
- **WHEN** `C-x C-x` is pressed and no mark was ever set
- **THEN** point does not move and the message is `No mark set in this buffer`

### Requirement: Changing the buffer deactivates the mark
A command that changes the text of the current buffer SHALL deactivate the mark, leaving it set.

#### Scenario: Typing after selecting
- **WHEN** `C-SPC C-f` is pressed at 0 in `hello` and then `x` is typed
- **THEN** the mark is at 0 and inactive and no region is shown

### Requirement: The mark ring belongs to the window's buffer
Showing another buffer in the window SHALL leave it with no mark and an empty mark ring.

#### Scenario: Visiting a file
- **WHEN** the mark is set in `*scratch*` and a file is visited
- **THEN** the window has no mark and an empty mark ring
