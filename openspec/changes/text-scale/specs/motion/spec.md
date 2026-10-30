# Spec Delta

## MODIFIED Requirements

### Requirement: Motion keys are bound by default
The global keymap SHALL bind `C-f` and `<right>` to `forward-char`, `C-b` and `<left>` to `backward-char`, `C-n` and `<down>` to `next-line`, `C-p` and `<up>` to `previous-line`, `M-f` to `forward-word`, `M-b` to `backward-word`, `C-a` to `move-beginning-of-line`, `C-e` to `move-end-of-line`, `M-<` and `M-S-<` to `beginning-of-buffer`, `M->` and `M-S->` to `end-of-buffer`, `C-v` to `scroll-up-command`, `M-v` to `scroll-down-command` and `C-l` to `recenter-top-bottom`.

#### Scenario: A motion key runs its command
- **WHEN** `C-f` is pressed
- **THEN** the command run is `forward-char`

#### Scenario: Shifted buffer-end keys
- **WHEN** `M-S-<` is pressed, as Linux reports `M-<`
- **THEN** the command run is `beginning-of-buffer`

### Requirement: The prefix argument counts motions
`forward-char`, `backward-char`, `next-line`, `previous-line`, `forward-word` and `backward-word` SHALL move by the prefix argument's value, 1 without one, and a negative value SHALL move the other way.

#### Scenario: C-u 3 C-f
- **WHEN** point is 0 in `abcdef` and `C-u 3 C-f` is pressed
- **THEN** point is 3

#### Scenario: C-u alone counts four
- **WHEN** point is 0 in `abcdef` and `C-u C-f` is pressed
- **THEN** point is 4

#### Scenario: Negative count reverses
- **WHEN** point is 5 in `abcdef` and `M-- 2 C-f` is pressed
- **THEN** point is 3

### Requirement: Char motion crosses lines and stops at the ends
`forward-char` and `backward-char` SHALL move by characters, a newline counting as one. A motion that would pass an end of the buffer SHALL stop there and report `End of buffer` or `Beginning of buffer`.

#### Scenario: Across a line end
- **WHEN** point is at the end of the first line of `ab\ncd` and `C-f` is pressed
- **THEN** point is at the start of the second line

#### Scenario: Past the end
- **WHEN** point is 1 in `abc` and `C-u 5 C-f` is pressed
- **THEN** point is 3 and the message is `End of buffer`

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

### Requirement: Word motion
`forward-word` SHALL move point past the end of the next word, and `backward-word` to the start of the previous one, where a word is a run of alphanumeric characters. At an end of the buffer they SHALL stop there without a message.

#### Scenario: Forward over words and punctuation
- **WHEN** point is 0 in `foo, bar baz` and `M-f M-f` is pressed
- **THEN** point is 8, after `bar`

#### Scenario: Backward with a count
- **WHEN** point is at the end of `foo, bar baz` and `C-u 2 M-b` is pressed
- **THEN** point is 5, at the start of `bar`

### Requirement: Line ends
`move-beginning-of-line` and `move-end-of-line` SHALL move point to the start or end of its line. With a prefix argument N other than 1 they SHALL first move N - 1 lines down, or up when N - 1 is negative, stopping at the buffer's first or last line.

#### Scenario: To the line's ends
- **WHEN** point is 4 in `ab\ncde` and `C-a` then `C-e` is pressed
- **THEN** point is first 3, then 6

#### Scenario: End of the next line
- **WHEN** point is 0 in `ab\ncde` and `C-u 2 C-e` is pressed
- **THEN** point is 6

### Requirement: Buffer ends
`beginning-of-buffer` and `end-of-buffer` SHALL move point to the start or end of the buffer. With a numeric prefix argument N (plain `C-u` counts as none) they SHALL instead move to N tenths of the buffer's length from the start or the end, then to the start of the next line. Unless the mark is active or the prefix is a plain `C-u`, they SHALL first push the mark at the old point without activating it and show `Mark set`.

#### Scenario: To the ends
- **WHEN** point is 2 in `ab\ncd` and `M->` then `M-<` is pressed
- **THEN** point is first 5, then 0, the mark is 5 and inactive, and the mark ring holds 2

#### Scenario: Tenths from the start
- **WHEN** the buffer is ten lines of `123456789\n`, 100 chars, and `C-u 5 M-<` is pressed
- **THEN** point is 60, the start of the line after the one holding char 50

#### Scenario: An active region keeps its mark
- **WHEN** `C-SPC` is pressed at 2 in `ab\ncd` and then `M->`
- **THEN** point is 5 and the mark is still 2 and active

### Requirement: Scrolling by screenfuls
`scroll-up-command` SHALL move the window's first line down by the lines it shows less 2, at least 1; `scroll-down-command` SHALL move it up the same way. With a numeric prefix argument they SHALL scroll that many lines, a negative count scrolling the other way, and with a lone minus a screenful the other way.

#### Scenario: A count scrolls lines
- **WHEN** a window showing 5 lines from line 0 of a 20-line buffer gets `C-u 2 C-v`
- **THEN** the window shows from line 2

### Requirement: Scrolling keeps to the buffer and point in view
When scrolling, the first line SHALL stay between the buffer's first and last lines; when it cannot move at all the command SHALL report `End of buffer` or `Beginning of buffer`. If point's line leaves the window, point SHALL move to the start of the nearest visible line.

#### Scenario: Scroll a screenful down and back
- **WHEN** a window showing 5 lines from line 0 of a 20-line buffer has point at 0, and `C-v` is pressed
- **THEN** the window shows from line 3 and point is at the start of line 3, and `M-v` shows from line 0 again with point still on line 3

#### Scenario: At the top
- **WHEN** the window shows from line 0 and `M-v` is pressed
- **THEN** nothing moves and the message is `Beginning of buffer`

### Requirement: Recentering
`recenter-top-bottom` SHALL scroll so point's line is in the middle row of the window; run again straight after itself, it SHALL put it at the top row, then the bottom row, then cycle. With a numeric prefix argument N it SHALL put point's line at row N, or N rows from the bottom when N is negative. A plain `C-u` SHALL recenter to the middle, and neither SHALL advance the cycle. The first line SHALL not go above the buffer's start.

#### Scenario: Cycle
- **WHEN** a window showing 5 lines of a 20-line buffer has point on line 10, and `C-l` is pressed three times
- **THEN** the window shows from line 8, then from line 10, then from line 6

#### Scenario: Row from the prefix
- **WHEN** a window showing 5 lines has point on line 10 and `C-u 1 C-l` is pressed
- **THEN** the window shows from line 9

### Requirement: Line motion and scrolling skip folded lines
A buffer line that starts inside invisible text (a fold) SHALL count as part of the line the fold starts on: `C-n` from that line SHALL go to the first line after the fold's end, `C-p` onto a folded line SHALL go to the fold's first line, and scrolling SHALL count a fold's lines as none, so a window keeps a fold's first line in view above point.

#### Scenario: Moving past a folded tool call
- **WHEN** point is on a folded tool call's header, whose content has 50 lines, and `C-n` is typed, then `C-p`
- **THEN** point goes to the blank line after the call, then back to the header

#### Scenario: Scrolling over a fold
- **WHEN** a window of 5 rows shows a session whose last block is a folded tool call with 50 lines of content, with point on the prompt line
- **THEN** the call's header is shown above the prompt line
