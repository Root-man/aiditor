# Spec Delta

## ADDED Requirements

### Requirement: Table rules are drawn
A table row SHALL be drawn with 1 px lines in the `separator` face's foreground: a vertical line at each cell boundary's x, across the row's full height so the lines of consecutive rows join, and a line along the row's bottom edge from its first boundary to its last. A table's first row SHALL also get a line along its top edge. A boundary past the window's text area SHALL NOT be drawn, and a bottom or top line SHALL stop at the text area's edge.

#### Scenario: A two-row table
- **WHEN** a table with boundaries at columns 0, 3 and 7 is drawn on two rows
- **THEN** 1 px `separator` lines run down both rows at the x of columns 0, 3 and 7, along the top of the first row and the bottom of each row, between the x of columns 0 and 7

#### Scenario: A table cut by the window
- **WHEN** a table's last boundary is past the window's right edge
- **THEN** the vertical lines that fit are drawn, and the row lines end at the text area's right edge
