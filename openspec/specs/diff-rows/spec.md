# diff-rows Specification

## Purpose
TBD - created by archiving change diff-rows. Update Purpose after archive.

## Requirements

### Requirement: Diff rows draw between a buffer's lines
A window SHALL draw its diff rows where they are anchored, before a buffer line or after it, with their own text runs and faces and their own gutter label, or a blank gutter when they have none. A row of height n with no text SHALL draw as n blank rows.

#### Scenario: Rows before the first line, between lines and after the last
- **WHEN** a window on a three-line buffer has a row before line 1, a row after line 2 and a row after line 3
- **THEN** its display shows the first row, line 1, line 2, the second row, line 3 and the third row, each row in its faces with its label in the gutter

#### Scenario: Filler
- **WHEN** a window has an empty row of height three between two lines
- **THEN** three blank rows show between them

#### Scenario: A long row wraps
- **WHEN** a row's text is wider than a window that wraps lines
- **THEN** it takes as many rows as a text line of that text would

### Requirement: Diff rows are not text
Point, line motion, search, the region and copying SHALL skip diff rows, and the buffer's text SHALL not change. The cursor, `hl-line`, the region and search highlights SHALL never draw on them.

#### Scenario: Moving past a row
- **WHEN** point is on line 2 and a row sits after it, and `C-n` runs
- **THEN** point is on line 3

#### Scenario: Copying across a row
- **WHEN** the region spans a row and is copied
- **THEN** the kill holds only the buffer's text

### Requirement: Scrolling counts diff rows
Scrolling, `C-l` and the number of buffer lines a window shows SHALL count each diff row as the rows it draws.

#### Scenario: Filler scrolls as its height
- **WHEN** a five-row window shows a buffer with an empty row of height three before line 3, and point moves to line 3
- **THEN** the window scrolls so that line 3 is its last row, with the three filler rows above it

### Requirement: Diff rows belong to a window
Diff rows SHALL belong to one window's view of a buffer: another window on the same buffer shows none of them, a split copies none, and showing another buffer drops them. Editing the buffer SHALL keep each row at the line it is anchored to; removing a window's rows SHALL leave its display as it was.

#### Scenario: Two windows on one buffer
- **WHEN** one of two windows on a buffer has rows
- **THEN** only that window shows them

#### Scenario: Inserting a line above
- **WHEN** a line is inserted above a row's anchor line
- **THEN** the row still draws next to the same text

#### Scenario: Removing the rows
- **WHEN** a window's rows are removed
- **THEN** its display is the same as before they were added
