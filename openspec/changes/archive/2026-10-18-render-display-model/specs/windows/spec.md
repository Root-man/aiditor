## MODIFIED Requirements

### Requirement: Splitting a window
`split-window-below` (`C-x 2`) SHALL split the selected window into two, one above the other, and `split-window-right` (`C-x 3`) into two side by side, each taking half of the old window's space, rounded to whole lines or cells. The new window SHALL be below or right of the old one, show the same buffer with the same point and first line, and have no mark. The old window SHALL stay selected.

#### Scenario: Split below
- **WHEN** `C-x 2` is typed with point at 3
- **THEN** there are two windows on the buffer, one above the other, both with point at 3, and the upper one is selected

#### Scenario: Splitting a split
- **WHEN** `C-x 2` is typed twice in a frame 10 lines high
- **THEN** the windows are 3, 2 and 5 lines high from the top
