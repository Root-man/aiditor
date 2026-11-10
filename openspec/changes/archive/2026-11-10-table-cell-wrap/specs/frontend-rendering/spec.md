# Spec Delta

## MODIFIED Requirements

### Requirement: Table rules are drawn
A table line SHALL be drawn with 1 px lines in the `separator` face's foreground: a vertical line at each cell boundary's x, across the line's full height so the lines of consecutive lines join, and, when the line is its row's last, a line along its bottom edge from its first boundary to its last. A table's first line SHALL also get a line along its top edge. A boundary past the window's text area SHALL NOT be drawn, and a bottom or top line SHALL stop at the text area's edge.

#### Scenario: A two-row table
- **WHEN** a table with boundaries at columns 0, 3 and 7 is drawn on two rows of one line each
- **THEN** 1 px `separator` lines run down both rows at the x of columns 0, 3 and 7, along the top of the first row and the bottom of each row, between the x of columns 0 and 7

#### Scenario: A row of two lines
- **WHEN** a table row's cells wrap onto two lines
- **THEN** the vertical lines run down both, and only the second has a line along its bottom

#### Scenario: A table cut by the window
- **WHEN** a table's last boundary is past the window's right edge
- **THEN** the vertical lines that fit are drawn, and the row lines end at the text area's right edge
