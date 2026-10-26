# windows Specification

## Purpose

Windows are viewports onto buffers: each holds point, mark and a scroll position, and keeps them right as the buffer is edited and point moves.

## Requirements

### Requirement: A window shows one buffer
The frame SHALL have one or more windows laid out by a split tree, one of them selected. Each window SHALL show one buffer. Commands SHALL act on the selected window and its buffer, and visiting a file SHALL show the new buffer in the selected window with point at the start and the scroll position at the first line, leaving the other windows as they were.

#### Scenario: Visiting a file
- **WHEN** a file is visited after typing in `*scratch*`
- **THEN** the selected window shows the file's buffer with point at 0

#### Scenario: Visiting a file in a split
- **WHEN** the frame is split and a file is visited
- **THEN** the selected window shows the file and the other window still shows `*scratch*`

### Requirement: Point and mark follow edits
A window's point and mark SHALL be positions in its buffer that follow edits: text inserted or deleted before them moves them, text inserted exactly at them leaves them before it unless the edit is a command typing at point, and a deletion around them moves them to its start. The mark SHALL be unset until a command sets it.

#### Scenario: An edit before point moves point
- **WHEN** point is at 3 in `abcdef` and `XY` is inserted at 1 without moving point
- **THEN** point is 5

#### Scenario: A deletion around point
- **WHEN** point is at 3 in `abcdef` and 1..5 is deleted
- **THEN** point is 1

#### Scenario: Mark follows edits
- **WHEN** the mark is set at 4 in `abcdef` and 0..2 is deleted
- **THEN** the mark is at 2

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

### Requirement: Splitting a window
`split-window-below` (`C-x 2`) SHALL split the selected window into two, one above the other, and `split-window-right` (`C-x 3`) into two side by side, each taking half of the old window's space, rounded to whole lines or cells. The new window SHALL be below or right of the old one, show the same buffer with the same point and first line, and have no mark. The old window SHALL stay selected.

#### Scenario: Split below
- **WHEN** `C-x 2` is typed with point at 3
- **THEN** there are two windows on the buffer, one above the other, both with point at 3, and the upper one is selected

#### Scenario: Splitting a split
- **WHEN** `C-x 2` is typed twice in a frame 10 lines high
- **THEN** the windows are 3, 2 and 5 lines high from the top

### Requirement: Windows keep separate points and scroll positions
Each window SHALL keep its own point, mark and first line, also when two windows show the same buffer. Edits made in one window SHALL move the other windows' points as edits made elsewhere do.

#### Scenario: Two windows on one buffer
- **WHEN** `C-x 2` is typed, point moves to the end in the upper window, then `C-x o` is typed
- **THEN** point is where it was before the motion, and `C-x o` again returns to the end

#### Scenario: Typing in one window moves the other's point
- **WHEN** both windows have point at 3 and `XY` is typed at 1 in the selected one
- **THEN** the other window's point is 5

### Requirement: Selecting another window
`other-window` (`C-x o`) SHALL select the next window in the split tree's order, top to bottom and left to right, wrapping from the last to the first. A prefix count N SHALL move N windows on, backward when N is negative. With one window it SHALL do nothing.

#### Scenario: Cycling through three windows
- **WHEN** there are three windows and `C-x o` is typed three times
- **THEN** the first window is selected again

#### Scenario: Backward
- **WHEN** the first of three windows is selected and `C-u - 1 C-x o` is typed
- **THEN** the third window is selected

### Requirement: Deleting windows
`delete-window` (`C-x 0`) SHALL delete the selected window, give its space to its sibling in the split tree and select the sibling's first window. Deleting the sole window SHALL fail with the message `Attempt to delete minibuffer or sole ordinary window`. `delete-other-windows` (`C-x 1`) SHALL delete every window but the selected one, which then fills the frame. A deleted window's point and mark SHALL no longer follow edits in its buffer.

#### Scenario: Delete the lower window
- **WHEN** the frame is split below, `C-x o` selects the lower window and `C-x 0` is typed
- **THEN** one window fills the frame and it is the upper one, with its point

#### Scenario: Delete the sole window
- **WHEN** there is one window and `C-x 0` is typed
- **THEN** the window stays and the message is `Attempt to delete minibuffer or sole ordinary window`

#### Scenario: Delete other windows
- **WHEN** there are three windows, the second is selected and `C-x 1` is typed
- **THEN** only the second window is left and it fills the frame
