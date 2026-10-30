# Spec Delta

## MODIFIED Requirements

### Requirement: Typing inserts at point
A printable key without Ctrl or Meta that no keymap binds SHALL run `self-insert-command`, which inserts its character at point the prefix argument's value times (1 without one) and moves point after the inserted text. A count of 0 SHALL insert nothing. A negative count SHALL insert nothing and report `Negative repetition argument N`. `RET` SHALL run `newline`, which inserts newlines the same way.

#### Scenario: Type two lines
- **WHEN** the keys `ab RET c` are pressed in an empty buffer
- **THEN** the text is `ab\nc` and point is at the end

#### Scenario: C-u 3 a
- **WHEN** `C-u 3 a` is pressed in an empty buffer
- **THEN** the text is `aaa`, point is 3 and the command run is `self-insert-command`

#### Scenario: C-u alone repeats four times
- **WHEN** `C-u RET` is pressed in an empty buffer
- **THEN** the text is four newlines and point is 4

#### Scenario: Negative count
- **WHEN** `M-- a` is pressed in an empty buffer
- **THEN** the text is empty and the message is `Negative repetition argument -1`

### Requirement: C-d deletes the character after point
`C-d` SHALL run `delete-char`, which deletes the prefix argument's value of characters (1 without one) after point, or before point when the value is negative. When fewer characters than that lie on that side of point, it SHALL delete nothing and report `End of buffer` or `Beginning of buffer`.

#### Scenario: Delete forward
- **WHEN** in `abc` with point at 0, `C-d` is pressed
- **THEN** the text is `bc` and point is 0

#### Scenario: Negative count deletes back
- **WHEN** in `abc` with point at 3, `M-- 2 C-d` is pressed
- **THEN** the text is `a` and point is 1

#### Scenario: Past the end
- **WHEN** in `abc` with point at 2, `C-u 2 C-d` is pressed
- **THEN** the text is `abc` and the message is `End of buffer`
