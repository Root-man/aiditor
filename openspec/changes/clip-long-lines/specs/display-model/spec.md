## MODIFIED Requirements

### Requirement: Point's line scrolls sideways to keep point visible
Given the window's width in columns, the display SHALL show point's display line from the window's horizontal scroll offset on, the cursor column counted from it. The offset SHALL be 0 while point's column fits the window, and otherwise SHALL move just far enough that the cursor is within the window's columns. Other lines SHALL be shown from column 0, truncated at the window edge. Each display line SHALL hold at most the window's columns plus one, the last one for a partly visible column; a display with no width SHALL cut nothing. A column is one char.

#### Scenario: End of a long line
- **WHEN** point moves with `C-e` to the end of a 100-char line in a window 40 columns wide
- **THEN** that line is shown from column 61 with the cursor at column 39, and the window's other lines start at column 0

#### Scenario: Back to a column that fits
- **WHEN** point then moves with `C-a` to the start of the line
- **THEN** the line is shown from column 0 with the cursor at column 0

#### Scenario: A line far wider than the window
- **WHEN** a buffer holds a 10 000-char line and the next line, and point is at the end of the long line in a window 20 columns wide
- **THEN** the long line holds its last 19 chars, from column 9 981, with the cursor at column 19, and with point on the next line the long line holds its first 21 chars

#### Scenario: The minibuffer
- **WHEN** the minibuffer's text is wider than the frame less its prompt
- **THEN** its line scrolls the same way to keep the cursor visible
