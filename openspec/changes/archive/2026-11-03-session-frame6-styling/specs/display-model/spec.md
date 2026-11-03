# Spec Delta

## ADDED Requirements

### Requirement: Lines can be indented
A buffer line SHALL be able to carry an indent of N columns as a line decoration. Its display rows SHALL hold N fewer columns, wrapping or cutting at that width, and SHALL report the indent so the frontend draws the text, the cursor and the line's background starting N columns further right. Point's column SHALL stay its char column in the line's text.

#### Scenario: An indented line
- **WHEN** a line with an indent of 4 is shown in a window of 40 columns that wraps
- **THEN** its rows hold 36 columns each and report an indent of 4

#### Scenario: The cursor on an indented line
- **WHEN** point is at char column 2 of a line indented 4
- **THEN** the display places the cursor at column 2 of that line, and the line reports an indent of 4

### Requirement: A session window has a prompt pane
The display of a window showing a session buffer SHALL carry the prompt pane's lines and cursor apart from the transcript's, with the pane's row count, as the session buffer's docking rule sets them. The transcript's lines SHALL fill the window's text rows less the pane's rows and its rule's height. The cursor SHALL be in the pane's lines when point is in the prompt area and in the transcript's otherwise.

#### Scenario: Point in the prompt
- **WHEN** a session window is displayed with point in the prompt area
- **THEN** the pane's lines hold the prompt line and the cursor, and the transcript's lines hold no cursor

#### Scenario: Other buffers
- **WHEN** a file buffer is displayed
- **THEN** its display carries no prompt pane

## MODIFIED Requirements

### Requirement: Text keeps clear of the window's sides
A window's text SHALL start 12 px from its left edge where it has no gutter, and SHALL stop at least 12 px before its right edge. A window showing a session buffer SHALL instead start its text 27 px from its left edge and stop it at least 24 px before its right edge. The display SHALL give each window the columns that leave, and the frame display SHALL report where the text starts.

#### Scenario: No gutter
- **WHEN** a window 100 px wide with no gutter is shown in 8 px cells
- **THEN** its lines hold 9 columns and its text starts at 12 px

#### Scenario: A session window
- **WHEN** a window 211 px wide showing a session buffer is shown in 8 px cells
- **THEN** its lines hold 20 columns and its text starts at 27 px

#### Scenario: The minibuffer
- **WHEN** a prompt is open in a frame 100 px wide with 8 px cells
- **THEN** the minibuffer's line scrolls in 9 columns less the prompt
