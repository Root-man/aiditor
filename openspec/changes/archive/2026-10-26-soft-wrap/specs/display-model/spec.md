## ADDED Requirements

### Requirement: Long lines wrap into visual rows
Unless its buffer truncates lines, a window at least 2 columns wide SHALL show each buffer line as visual rows of at most its width less one char, each a display line carrying the buffer line's number and decorations. A session buffer and a buffer visiting a `.md`, `.markdown` or `.txt` file SHALL break a row after the last whitespace that fits, a whitespace char also fitting in the last column, and at the width when no whitespace fits. Every other buffer SHALL break at the width and end each row but a line's last with a `\` continuation marker in the last column, in the theme's `comment` colour. The window's first row MAY be a row inside its first buffer line. The cursor SHALL be on the row holding point, at point's column from the row's start; point at a break SHALL be at the start of the next row. Only the chars from a line's start through the rows the window can show SHALL be laid out.

#### Scenario: Code wraps at the edge
- **WHEN** `a.rs` holds a 10-char line `abcdefghij` in a window 5 columns wide
- **THEN** the display lines read `abcd\`, `efgh\` and `ij`, all buffer line 0

#### Scenario: Markdown wraps at words
- **WHEN** `a.md` holds `one two three` in a window 9 columns wide
- **THEN** the display lines read `one two ` and `three`

#### Scenario: A word longer than the window
- **WHEN** `a.md` holds `abcdefghij` in a window 5 columns wide
- **THEN** the display lines read `abcd`, `efgh` and `ij`

#### Scenario: The cursor on a wrapped line
- **WHEN** point is at column 5 of `abcdefghij` in `a.rs` in a window 5 columns wide
- **THEN** the cursor is on the second row at column 1

#### Scenario: A short line
- **WHEN** a line fits in the window less one column
- **THEN** it is one display line without a marker

### Requirement: toggle-truncate-lines switches a buffer between wrapping and truncation
`toggle-truncate-lines` SHALL switch the current buffer between wrapping and truncating long lines and echo `Truncate long lines enabled` or `Truncate long lines disabled`. Buffers SHALL wrap until toggled. The minibuffer SHALL always truncate.

#### Scenario: Toggling to truncation and back
- **WHEN** `M-x toggle-truncate-lines` runs in a buffer with a line wider than its window
- **THEN** the echo area says `Truncate long lines enabled` and the line is one display line cut at the window edge
- **WHEN** it runs again
- **THEN** the echo area says `Truncate long lines disabled` and the line wraps again

## MODIFIED Requirements

### Requirement: Point's line scrolls sideways to keep point visible
In a buffer that truncates lines, given the window's width in columns, the display SHALL show point's display line from the window's horizontal scroll offset on, the cursor column counted from it. The offset SHALL be 0 while point's column fits the window, and otherwise SHALL move just far enough that the cursor is within the window's columns. Other lines SHALL be shown from column 0, truncated at the window edge. Each display line SHALL hold at most the window's columns plus one, the last one for a partly visible column; a display with no width SHALL cut nothing. A column is one char. A buffer that wraps SHALL have an offset of 0.

#### Scenario: End of a long line
- **WHEN** point moves with `C-e` to the end of a 100-char line in a window 40 columns wide, in a buffer that truncates lines
- **THEN** that line is shown from column 61 with the cursor at column 39, and the window's other lines start at column 0

#### Scenario: Back to a column that fits
- **WHEN** point then moves with `C-a` to the start of the line
- **THEN** the line is shown from column 0 with the cursor at column 0

#### Scenario: A line far wider than the window
- **WHEN** a buffer that truncates lines holds a 10 000-char line and the next line, and point is at the end of the long line in a window 20 columns wide
- **THEN** the long line holds its last 19 chars, from column 9 981, with the cursor at column 19, and with point on the next line the long line holds its first 21 chars

#### Scenario: The minibuffer
- **WHEN** the minibuffer's text is wider than the frame less its prompt
- **THEN** its line scrolls the same way to keep the cursor visible
