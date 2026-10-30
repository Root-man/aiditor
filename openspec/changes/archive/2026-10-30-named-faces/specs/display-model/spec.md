## MODIFIED Requirements

### Requirement: The region is highlighted
When the mark is set and active, the text between point and mark SHALL be drawn in the theme's `region` face, over any other face.

#### Scenario: Region across a face
- **WHEN** the mark is active at 0 and point at 3 in `hello`
- **THEN** `hel` has the `region` background and `lo` the default face

#### Scenario: No mark
- **WHEN** the mark is unset
- **THEN** no span has the `region` background

#### Scenario: Inactive mark
- **WHEN** the mark is at 0 and inactive and point at 3 in `hello`
- **THEN** no span has the `region` background

### Requirement: Long lines wrap into visual rows
Unless its buffer truncates lines, a window at least 2 columns wide SHALL show each buffer line as visual rows of at most its width less one char, each a display line carrying the buffer line's number and decorations. A session buffer and a buffer visiting a `.md`, `.markdown` or `.txt` file SHALL break a row after the last whitespace that fits, a whitespace char also fitting in the last column, and at the width when no whitespace fits. Every other buffer SHALL break at the width and end each row but a line's last with a `\` continuation marker in the last column, in the theme's `comment` syntax face. The window's first row MAY be a row inside its first buffer line. The cursor SHALL be on the row holding point, at point's column from the row's start; point at a break SHALL be at the start of the next row. Only the chars of the rows the window can show SHALL be laid out: the rows of the first buffer line above the window's first row SHALL NOT be.

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

#### Scenario: The window starts deep inside a line
- **WHEN** `a.md` holds 1,000 copies of `word ` on one line, a window 21 columns wide shows 3 rows and point is at the line's end
- **THEN** the display lines are the line's last rows as wrapping the whole line at words would break them, the cursor is on the last row after its final char, and laying out the display reads none of the line's chars before the first row shown
