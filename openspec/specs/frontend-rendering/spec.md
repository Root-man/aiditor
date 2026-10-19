# frontend-rendering Specification

## Purpose
How the frontend draws the core's frame display: every window's lines, cursor, region and mode line, plus the echo area, the key hint and the debug stats, so what is on screen is exactly the display model.

## Requirements

### Requirement: Every window is drawn at its rectangle
The frontend SHALL draw every window of the frame display at its rectangle, clipped to it, with its lines from the top and its mode line on its last line. Resizing the OS window SHALL re-lay out and redraw every window.

#### Scenario: A side-by-side split
- **WHEN** `C-x 3` is typed
- **THEN** both windows are drawn side by side, each with its own text and mode line, and a long line in the left window does not draw into the right one

#### Scenario: Resizing the frame
- **WHEN** the OS window is made taller with two stacked windows
- **THEN** both windows grow and show more lines

### Requirement: Lines are drawn whole with their spans' faces
Each display line SHALL be laid out as one piece of text in the settings' font family and size, each span in its face's foreground and background colors, the default foreground where a face sets none. A line background decoration SHALL fill the line's width and a rule decoration SHALL be drawn at the line's left edge.

#### Scenario: Region shown
- **WHEN** the mark is active at 0 and point at 3 in `hello`
- **THEN** `hel` is drawn on the region color and `lo` on the background

### Requirement: The cursor is drawn over the line without moving its text
The selected window SHALL draw the cursor as a block in the theme's cursor color at the x offset of point's column in its laid-out line, over the text, as wide as the char at point or one cell at the end of the line, with that char drawn over the block in the background color. Point's line SHALL be laid out exactly as any other line, so moving point never shifts any glyph. Other windows SHALL draw no cursor.

#### Scenario: Moving point keeps glyphs still
- **WHEN** point moves from the start to the middle of a line
- **THEN** the line's laid-out text is the same and the cursor moves to that column's x offset

#### Scenario: A block over the char
- **WHEN** point is before a char
- **THEN** the cursor is as wide as that char

### Requirement: The mode line marks the selected window
Each window's mode line SHALL be drawn on the theme's mode line color for the selected window and on its inactive mode line color for the others.

#### Scenario: Two windows
- **WHEN** the frame is split with `C-x 2`
- **THEN** the upper mode line uses the mode line color and the lower one the inactive color

### Requirement: The echo area is always shown
The frame SHALL end with one echo area line, showing the core's message or blank when there is none, so messages never resize the windows.

#### Scenario: A message appears
- **WHEN** `C-g` shows `Quit`
- **THEN** `Quit` is drawn in the echo area and the windows keep their sizes

### Requirement: The key hint is laid out in columns
While the key hint shows, it SHALL be drawn above the echo area as the pending keys then each entry, `key : command` with the keys right-aligned so the colons line up, in columns of equal width, as many per row as fit the frame's width and as many rows as needed. The windows SHALL shrink to make room.

#### Scenario: C-x hint
- **WHEN** `C-x` waits past the hint delay in a wide frame
- **THEN** its entries, such as `u : undo`, are drawn in several columns above the echo area

### Requirement: Debug stats include shaping time
With `--debug`, the stats line at the top SHALL show the frame rate, render, shaping and core handling times and the Tokio tick latency, the shaping time being how long laying out the last frame's lines took.

#### Scenario: Debug run
- **WHEN** the editor runs with `--debug` and point moves
- **THEN** the stats line shows a shaping time in ms

### Requirement: The frame keeps a gap at its edges
The frame's content SHALL be inset 8 px from the left and right edges and 2 px from the bottom, so rounded window corners never clip text.

#### Scenario: Minibuffer at the corner
- **WHEN** `M-x` is typed
- **THEN** the prompt's first letter starts 8 px from the frame's left edge
