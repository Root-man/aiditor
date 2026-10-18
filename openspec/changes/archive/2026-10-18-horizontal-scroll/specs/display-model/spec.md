## ADDED Requirements

### Requirement: Point's line scrolls sideways to keep point visible
Given the window's width in columns, the display SHALL show point's display line from the window's horizontal scroll offset on, the cursor column counted from it. The offset SHALL be 0 while point's column fits the window, and otherwise SHALL move just far enough that the cursor is within the window's columns. Other lines SHALL be shown from column 0, truncated at the window edge. A column is one char.

#### Scenario: End of a long line
- **WHEN** point moves with `C-e` to the end of a 100-char line in a window 40 columns wide
- **THEN** that line is shown from column 61 with the cursor at column 39, and the window's other lines start at column 0

#### Scenario: Back to a column that fits
- **WHEN** point then moves with `C-a` to the start of the line
- **THEN** the line is shown whole from column 0 with the cursor at column 0

#### Scenario: The minibuffer
- **WHEN** the minibuffer's text is wider than the frame less its prompt
- **THEN** its line scrolls the same way to keep the cursor visible

### Requirement: A click on the scrolled line lands on the clicked char
A click on point's line SHALL count its column from the line's horizontal scroll offset.

#### Scenario: Click after C-e
- **WHEN** a line is shown from column 61 and the shown column 0 is clicked
- **THEN** point moves to buffer column 61 of that line
