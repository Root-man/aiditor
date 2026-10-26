## MODIFIED Requirements

### Requirement: Scrolling keeps point visible
Given how many rows fit in the window, the window's first row SHALL move only as far as needed for point's row to be visible: down when point is below the last visible row, up when it is above the first. Rows are visual rows: a wrapped buffer line counts one per row it wraps into, a line that is not wrapped counts one.

#### Scenario: Point moves below the window
- **WHEN** the window fits 3 lines, shows from line 0, and point moves to line 5
- **THEN** the window shows from line 3

#### Scenario: Point moves back up
- **WHEN** the window shows from line 3 and point moves to line 1
- **THEN** the window shows from line 1

#### Scenario: Point stays on screen
- **WHEN** point moves within the visible lines
- **THEN** the first line does not change

#### Scenario: Point at the end of a wrapped line
- **WHEN** a window 3 rows high and 5 columns wide shows a buffer whose line 0 wraps into 4 rows, and point moves to the end of line 0
- **THEN** the window shows from line 0's second row, and its last row holds the cursor

#### Scenario: Point below wrapped lines
- **WHEN** lines 0 and 1 each wrap into 2 rows in a window 3 rows high, and point moves to line 2
- **THEN** the window shows from line 1's first row
