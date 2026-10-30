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

### Requirement: DEL deletes the character before point
`DEL` SHALL run `delete-backward-char`, which deletes the prefix argument's value of characters (1 without one) before point, or after point when the value is negative. When fewer characters than that lie on that side of point, it SHALL delete nothing and report `Beginning of buffer` or `End of buffer`.

#### Scenario: Delete back
- **WHEN** the keys `abc DEL` are pressed in an empty buffer
- **THEN** the text is `ab` and point is 2

#### Scenario: Delete back by a count
- **WHEN** the keys `abcd C-u 2 DEL` are pressed in an empty buffer
- **THEN** the text is `ab` and point is 2

#### Scenario: At the start
- **WHEN** `DEL` is pressed with point at 0
- **THEN** the text and point do not change and the message is `Beginning of buffer`

### Requirement: DEL and C-d delete an active region
Without a prefix argument, `delete-backward-char` and `delete-char` SHALL delete the region instead when the mark is active and the region is not empty, without saving it to the kill ring, and point SHALL end at the region's start.

#### Scenario: Delete the selected line
- **WHEN** in `hello\nx` with point at 0, `C-e C-SPC C-a DEL` is pressed
- **THEN** the text is `\nx`, point is 0 and the mark is inactive

#### Scenario: A prefix deletes characters
- **WHEN** in `hello` with point at 0, `C-SPC C-f C-f C-f C-u 1 DEL` is pressed
- **THEN** the text is `helo`

### Requirement: C-j inserts a newline
`C-j` SHALL run `electric-newline-and-maybe-indent`, which inserts newlines like `newline` until indentation exists.

#### Scenario: C-u 2 C-j
- **WHEN** `a C-u 2 C-j` is pressed in an empty buffer
- **THEN** the text is `a\n\n` and point is 3

### Requirement: C-o opens a line
`C-o` SHALL run `open-line`, which inserts the prefix argument's value of newlines (1 without one) at point and leaves point before them. A negative count SHALL insert nothing and report `Negative repetition argument N`.

#### Scenario: Open two lines
- **WHEN** in `ab` with point at 1, `C-u 2 C-o` is pressed
- **THEN** the text is `a\n\nb` and point is 1

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

### Requirement: M-d and M-DEL delete words
`M-d` SHALL run `kill-word`, which kills from point to where `forward-word` with the same prefix argument would move. `M-DEL` SHALL run `backward-kill-word`, which does the same as `backward-word` would move. The killed text SHALL be saved on the kill ring like any other kill.

#### Scenario: Kill a word forward
- **WHEN** in `foo bar baz` with point at 0, `M-d` is pressed
- **THEN** the text is ` bar baz` and point is 0

#### Scenario: Kill words forward by a count
- **WHEN** in `foo bar baz` with point at 0, `C-u 2 M-d` is pressed
- **THEN** the text is ` baz`

#### Scenario: Kill a word backward
- **WHEN** in `foo bar` with point at 7, `M-DEL` is pressed
- **THEN** the text is `foo ` and point is 4

#### Scenario: The killed word yanks back
- **WHEN** in `foo bar` with point at 7, `M-DEL C-y` is pressed
- **THEN** the text is `foo bar`
