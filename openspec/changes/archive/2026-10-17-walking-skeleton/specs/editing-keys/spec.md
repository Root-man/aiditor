# Spec Delta

## Purpose

The keys that insert text, delete it and move point in the current buffer, before the command registry and keymaps exist.

## ADDED Requirements

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

### Requirement: Arrow keys move point
Left and right SHALL move point by one character, crossing line ends, and stop at the buffer's ends. Up and down SHALL move to the same column on the previous or next line, clamped to that line's length, and do nothing on the first or last line.

#### Scenario: Left and right
- **WHEN** point is at the start of the second line of `ab\ncd` and left is pressed
- **THEN** point is at the end of the first line, and right moves it back

#### Scenario: Ends of the buffer
- **WHEN** left is pressed at the start of the buffer, or right at its end
- **THEN** point does not move

#### Scenario: Up and down keep the column, clamped
- **WHEN** point is at column 3 of `abcd\nx\nwxyz` line 0 and down is pressed twice
- **THEN** point is first at the end of `x`, then at column 1 of `wxyz`

#### Scenario: First and last line
- **WHEN** up is pressed on the first line, or down on the last
- **THEN** point does not move
