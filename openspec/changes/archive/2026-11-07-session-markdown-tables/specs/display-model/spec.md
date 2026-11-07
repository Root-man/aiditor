# Spec Delta

## ADDED Requirements

### Requirement: Table lines are cut, not wrapped
A line carrying the table decoration SHALL be one display line in every buffer, wrapping or not, holding at most the window's columns plus one for a partly visible last column. It SHALL get no continuation marker. Other lines of the buffer SHALL wrap as before.

#### Scenario: A wide table in a narrow window
- **WHEN** a session buffer holds a 71-char table line and a 71-char prose line, in a window 48 columns wide
- **THEN** the table line is one display line holding its first 49 chars and the prose line wraps into two rows

### Requirement: A table scrolls sideways as one
When point is on a table line, every line of the run of consecutive table lines holding it SHALL be shown from one horizontal offset, chosen as for point's line in a buffer that truncates lines. The cursor column SHALL be counted from that offset. Lines outside that run SHALL be shown from column 0, and leaving the table SHALL reset the offset to 0.

#### Scenario: End of a table row
- **WHEN** point moves with `C-e` to the end of a 71-char table row in a window 48 columns wide, in a table of three lines
- **THEN** all three table lines are shown from the same column, the cursor is in the window's last column, and the prose line after the table starts at column 0

#### Scenario: Leaving the table
- **WHEN** point then moves with `C-n` past the table's last line
- **THEN** the table lines are shown from column 0 again

## MODIFIED Requirements

### Requirement: Lines carry their decorations
A line decoration property SHALL apply to every display line its range touches. A decoration SHALL be a background color for the whole line, a rule beside it in a color, a 1 px border at the line's top or bottom in a color, a 1 px horizontal rule through the middle of the line's empty columns in a color, or a table row: the columns of its cell boundaries and whether it is its table's first row.

#### Scenario: A code block background
- **WHEN** lines 1 and 2 have a background decoration
- **THEN** those display lines carry it and lines 0 and 3 do not

#### Scenario: Borders and a horizontal rule
- **WHEN** line 1 has a top border decoration and line 2 a horizontal rule decoration
- **THEN** display line 1 carries the border and display line 2 the rule, and no other line carries either

#### Scenario: A table row
- **WHEN** line 1 has a table decoration with boundaries 0, 3 and 7, marked first
- **THEN** display line 1 carries it with those boundaries, shifted left by the line's horizontal offset
