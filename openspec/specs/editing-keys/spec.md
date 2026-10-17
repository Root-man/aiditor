# editing-keys Specification

## Purpose
The commands that insert and delete text at point, the keys bound to them, and typing, each honouring the prefix argument as Emacs does.

## Requirements

### Requirement: Typing inserts at point
A printable key without Ctrl or Meta SHALL insert its character at point and move point after it; `RET` SHALL insert a newline the same way.

#### Scenario: Type two lines
- **WHEN** the keys `ab RET c` are pressed in an empty buffer
- **THEN** the text is `ab\nc` and point is at the end

### Requirement: DEL deletes the character before point
`DEL` SHALL delete the character before point and move point back by one; at the start of the buffer it SHALL do nothing.

#### Scenario: Delete back
- **WHEN** the keys `abc DEL` are pressed in an empty buffer
- **THEN** the text is `ab` and point is 2

#### Scenario: At the start
- **WHEN** `DEL` is pressed with point at 0
- **THEN** the text and point do not change
