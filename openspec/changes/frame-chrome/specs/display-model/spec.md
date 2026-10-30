# Spec Delta

## MODIFIED Requirements

### Requirement: Visible lines fill the window
Given the window's height in pixels, the display SHALL list the lines shown from the window's first line down, each with its buffer line number and height, as many as fit between the 6 px under the window's top and the mode line. Every line SHALL be one line height (the font size times `line_height`) while the font is monospace. Point's line SHALL be among them.

#### Scenario: A short buffer
- **WHEN** a three-line buffer is shown in a window ten lines high
- **THEN** the display has lines 0, 1 and 2

#### Scenario: A long buffer scrolled to point
- **WHEN** a window fits three lines plus the mode line and point is on line 5
- **THEN** the display shows lines 3, 4 and 5

### Requirement: The frame display lays windows out by the split tree
Given the frame's width and height in pixels and the font's cell width, as last reported by the frontend, the frame display SHALL list every window in the split tree's order with its id, its rectangle, whether it is selected, and its window display built for the rectangle's height. A stacked split SHALL give its first part the split's ratio of the space with its text area rounded to whole lines, and a side-by-side split the ratio rounded to whole cells, the second part taking the rest less a 1 px divider between side-by-side parts. The rectangles SHALL tile the frame without overlap.

#### Scenario: A stacked split
- **WHEN** a frame 800 by 600 px is split with `C-x 2`
- **THEN** the frame display has a selected window at 0,0 whose height is whole lines plus the chrome and another below it taking the rest of the 600 px, each with its lines and mode line

#### Scenario: A side-by-side split
- **WHEN** a frame 800 by 600 px with 8 px cells is split with `C-x 3`
- **THEN** the frame display has a selected window at 0,0 sized 400 by 600 and another at 401,0 sized 399 by 600

#### Scenario: Resizing re-lays out
- **WHEN** the frontend reports a new frame size
- **THEN** the next frame display lays the windows out in that size

## ADDED Requirements

### Requirement: Text keeps clear of the window's sides
A window's text SHALL start 12 px from its left edge where it has no gutter, and SHALL stop at least 12 px before its right edge. The display SHALL give each window the columns that leave, and the frame display SHALL report where the text starts.

#### Scenario: No gutter
- **WHEN** a window 100 px wide with no gutter is shown in 8 px cells
- **THEN** its lines hold 9 columns and its text starts at 12 px

#### Scenario: The minibuffer
- **WHEN** a prompt is open in a frame 100 px wide with 8 px cells
- **THEN** the minibuffer's line scrolls in 9 columns less the prompt
