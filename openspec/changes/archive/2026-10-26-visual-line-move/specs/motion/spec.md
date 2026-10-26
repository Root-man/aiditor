## MODIFIED Requirements

### Requirement: Line motion keeps a goal column
`next-line` and `previous-line` SHALL move to the same column on the target line, clamped to its length. The column SHALL be the one point had when the first of a run of consecutive `next-line` and `previous-line` commands started, so a short line in between does not lose it. In a buffer whose window wraps long lines, they SHALL move by visual rows instead, as Emacs's `line-move-visual`: the goal column SHALL be point's column within its row, and the target SHALL be that column of the target row, clamped to the row's last char, or to the line's end on the line's last row. A count SHALL move that many rows. A buffer that truncates long lines, and the minibuffer, SHALL move by buffer lines. `move-beginning-of-line`, `move-end-of-line` and `kill-line` SHALL keep acting on buffer lines.

#### Scenario: Through a short line
- **WHEN** point is at column 3 of the first line of `abcd\nx\nwxyz` and `C-n C-n` is pressed
- **THEN** point is first at the end of `x`, then at column 3 of `wxyz`

#### Scenario: Another command resets the goal
- **WHEN** in `abcd\nx\nwxyz` point is at column 3 of the first line, `C-n` is pressed, then `C-b`, then `C-n`
- **THEN** point ends at the start of `wxyz`

#### Scenario: Through a wrapped line
- **WHEN** a window wraps `abcdefghij\nxy` into rows `abcd`, `efgh`, `ij`, `xy`, point is 1 and `C-n` is pressed three times
- **THEN** point is 5, then 9, then 12, and `C-p` three times brings it back through 9 and 5 to 1

#### Scenario: A short row clamps to its last char
- **WHEN** a window word-wraps `ab cdefghij` into rows `ab ` and `cdefghij`, point is 8 (column 5 of the second row) and `C-p` is pressed
- **THEN** point is 2, on the space ending the first row

#### Scenario: A count moves rows
- **WHEN** a window wraps `abcdefghij` into rows `abcd`, `efgh`, `ij`, point is 0 and `C-u 2 C-n` is pressed
- **THEN** point is 8

#### Scenario: Line ends stay logical
- **WHEN** a window wraps `abcdefghij` into rows `abcd`, `efgh`, `ij` and point is 5
- **THEN** `C-a` moves point to 0 and `C-e` to 10

#### Scenario: A truncating buffer moves by lines
- **WHEN** `toggle-truncate-lines` truncates `abcdefghij\nxy`, point is 1 and `C-n` is pressed
- **THEN** point is 12

### Requirement: Line motion stops at the ends
When `next-line` or `previous-line` would pass the last or first line, or the last or first row in a buffer that wraps, point SHALL move to the end or start of the buffer and report `End of buffer` or `Beginning of buffer`.

#### Scenario: Down on the last line
- **WHEN** point is 1 in `ab\ncd` and `C-u 3 C-n` is pressed
- **THEN** point is 5 and the message is `End of buffer`

#### Scenario: Up on the first line
- **WHEN** point is 1 in `ab\ncd` and `C-p` is pressed
- **THEN** point is 0 and the message is `Beginning of buffer`

#### Scenario: Down on the last row of a wrapped line
- **WHEN** a window wraps `abcdefghij` into rows `abcd`, `efgh`, `ij`, point is 8 and `C-n` is pressed
- **THEN** point is 10 and the message is `End of buffer`
