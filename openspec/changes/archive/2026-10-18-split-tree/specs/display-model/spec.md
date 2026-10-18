## ADDED Requirements

### Requirement: The frame display lays windows out by the split tree
Given the frame's width and height in pixels, the frame display SHALL list every window in the split tree's order with its rectangle, whether it is selected, and its window display built for the rectangle's height. A split SHALL give its first part the split's ratio of the space, stacked splits dividing the height and side-by-side splits the width, and the rectangles SHALL tile the frame without overlap.

#### Scenario: A stacked split
- **WHEN** a frame 800 by 600 px is split with `C-x 2`
- **THEN** the frame display has a selected window at 0,0 sized 800 by 300 and another at 0,300 sized 800 by 300, each with its lines and mode line

#### Scenario: A side-by-side split
- **WHEN** a frame 800 by 600 px is split with `C-x 3`
- **THEN** the frame display has a selected window at 0,0 sized 400 by 600 and another at 400,0 sized 400 by 600
