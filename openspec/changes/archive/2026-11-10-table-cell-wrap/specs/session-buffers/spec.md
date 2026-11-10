# Spec Delta

## MODIFIED Requirements

### Requirement: Tables render as drawn text
A GFM pipe table at the top level of an agent message block SHALL be replaced in the buffer by drawn text: one line for the header and one for each body row, or more when the row's cells wrap at `table_width`, without the delimiter row. Each cell SHALL be a space, its text padded to its column's width and a space. A column's natural width SHALL be its widest cell's shown text, measured in chars, and its width the natural one unless the table wraps. Cells SHALL align left, centre or right as the delimiter row says. Each line SHALL carry the table line decoration.

#### Scenario: Columns and alignment
- **WHEN** the agent streams `| a | b |\n|---|--:|\n| x | 10 |\n` and a frame is built
- **THEN** the block's lines read ` a   b ` and ` x  10 `, and both carry the table decoration

#### Scenario: A ragged table
- **WHEN** a table's header has three cells, one body row two and another four
- **THEN** each drawn line has three cells: the short row ends in an empty cell and the long row's fourth cell is dropped

#### Scenario: A nested table stays text
- **WHEN** the agent streams a table inside a block quote or a list item
- **THEN** its lines stay as written, pipes and delimiter row included

#### Scenario: A table still streaming
- **WHEN** a table's header and delimiter row have arrived, a frame is built, then a row arrives and another frame is built
- **THEN** the first frame shows the header line alone, and the second also shows the row, with columns widened to fit it

## ADDED Requirements

### Requirement: Wide tables wrap their cells
A drawn table's width, its cells and their spaces, SHALL be at most `table_width` chars when its columns can each be at least one char wide. A table whose natural width passes it SHALL keep at their natural width the columns that fit an equal share of the room left by the narrower ones, and share that room evenly among the rest, the first columns taking any remainder. A cell longer than its column SHALL wrap at spaces onto more lines of the same row, a word longer than the column broken at its width; the spaces at a break SHALL not show. A row SHALL take as many lines as its tallest cell, the other cells empty on the lines they don't fill. Alignment SHALL apply to each line of a cell. Each line SHALL carry the table line decoration, marked as its row's last line only on the last. The cap SHALL NOT depend on the window.

#### Scenario: A long cell wraps at words
- **WHEN** `table_width` is 20 and the agent streams `| k | v |\n|---|---|\n| a | one two three four |\n`
- **THEN** column `k` stays 1 wide and column `v` is 15 wide, and the body row takes two lines, ` a  one two three` and `    four`, each padded to 20 chars

#### Scenario: A long word is broken
- **WHEN** a cell's word is longer than its column
- **THEN** it is cut at the column's width and goes on at the start of the row's next line

#### Scenario: Alignment on each line
- **WHEN** a right-aligned column's cell wraps
- **THEN** each of its lines ends at the column's right edge

#### Scenario: A table that fits
- **WHEN** a table's natural width is at most `table_width`
- **THEN** it is drawn one line per row, as before

#### Scenario: A wrapped table still streaming
- **WHEN** a wide table's rows arrive across frames
- **THEN** each frame draws the rows so far, wrapped at the cap
