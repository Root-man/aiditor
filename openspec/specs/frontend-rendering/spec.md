# frontend-rendering Specification

## Purpose
How the frontend draws the core's frame display: every window's lines, cursor, region and mode line, plus the echo area, the key hint and the debug stats, so what is on screen is exactly the display model.

## Requirements

### Requirement: Every window is drawn at its rectangle
The frontend SHALL draw every window of the frame display at its rectangle, clipped to it, with its lines from its top and its mode line along its bottom, over any row it cuts off. Resizing the OS window SHALL re-lay out and redraw every window.

#### Scenario: A side-by-side split
- **WHEN** `C-x 3` is typed
- **THEN** both windows are drawn side by side, each with its own text and mode line, and a long line in the left window does not draw into the right one

#### Scenario: Resizing the frame
- **WHEN** the OS window is made taller with two stacked windows
- **THEN** both windows grow and show more lines

#### Scenario: Flush with the bar
- **WHEN** a window is drawn
- **THEN** its first row starts at the window's top

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
Each window's mode line SHALL be drawn on the theme's `mode-line` face's background for the selected window and on its `mode-line-inactive` face's for the others, with the left group's text at the left inset and the right group's ending at the right inset, each span in its own face. An inactive mode line SHALL have a 1 px rule along its top in the `mode-line` face's background.

#### Scenario: Two windows
- **WHEN** the frame is split with `C-x 2`
- **THEN** the upper mode line uses the `mode-line` face and the lower one `mode-line-inactive`

#### Scenario: The inactive rule
- **WHEN** the frame is split with `C-x 2`
- **THEN** only the inactive mode line has a 1 px rule, inside its height along its top edge

#### Scenario: The right group
- **WHEN** a window's mode line is drawn
- **THEN** its right group ends 12 px from the window's right edge

### Requirement: The echo area is always shown
The frame SHALL end with one echo area line, as high as a mode line, showing the core's message or blank when there is none, so messages never resize the windows.

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

### Requirement: Terminal cells are drawn on the grid
A terminal window SHALL draw each run at its column times the cell width, each glyph advancing one cell, so wide chars and glyphs from fallback fonts never shift the rest of the row. A run's background SHALL fill its cells. Bold and italic runs SHALL use the font's bold weight and italic style, underlined runs an underline.

#### Scenario: A wide char keeps the row aligned
- **WHEN** a terminal shows `日本x` above `abcde`
- **THEN** `x` is drawn at the same x as `e`

### Requirement: The terminal cursor is drawn in its shape
A terminal window's cursor SHALL be drawn in the theme's cursor colour as a block over its cells, with the char under it in the background colour, as a beam at the cell's left edge, or as an underline at the cell's bottom, as the display says. In a window that isn't selected it SHALL be drawn as a hollow block outline whatever its shape. With no cursor in the display, none SHALL be drawn.

#### Scenario: Unselected window
- **WHEN** a terminal is shown in two windows
- **THEN** the selected one draws its cursor filled and the other draws a hollow outline

### Requirement: Emoji are drawn
Emoji SHALL be drawn wherever the frontend draws text. When the colour emoji font can't be drawn, having neither colour bitmaps nor COLR v0 layers, emoji SHALL be drawn in the text's colour from another installed font that has them, such as Noto Emoji or Symbola. A colour emoji font that can be drawn SHALL be used as is.

#### Scenario: Crab on Fedora
- **WHEN** a file containing `🦀` is shown, the only colour emoji font is COLRv1 and Noto Emoji is installed
- **THEN** the crab is drawn, monochrome, in the foreground colour

#### Scenario: Crab with a drawable colour font
- **WHEN** a file containing `🦀` is shown on macOS
- **THEN** the crab is drawn in colour from Apple Color Emoji

### Requirement: The workspace modeline is drawn above every window
The frontend SHALL draw one workspace modeline row above the window tree, its background spanning the frame edge to edge while its text keeps the 12 px inset, in the editor's font at the workspace font size and that size's line height plus 4 px, rounded, showing the project name bold, ` › ` in the `separator` face and the workspace name, in the `workspace-bar` face. The windows SHALL be laid out below it, so it never covers buffer text, in windowed and fullscreen frames alike.

#### Scenario: One bar above the splits
- **WHEN** the frame shows two side-by-side windows in workspace `main` of project `home`
- **THEN** one row reading `home › main` is drawn above both windows, from the frame's left edge to its right, and the windows start below it

#### Scenario: Design size
- **WHEN** `workspace_font_size` is 13 and `line_height` is 1.4
- **THEN** the workspace modeline is 22 px high

### Requirement: Text is inset from the frame's edges
Mode lines, the echo line, minibuffer rows, key hint rows and the debug stats SHALL draw their text 12 px from the frame's or window's left edge. Their backgrounds and windows SHALL reach the frame's edges, with no gap.

#### Scenario: Minibuffer at the corner
- **WHEN** `M-x` is typed
- **THEN** the prompt's first letter starts 12 px from the frame's left edge

#### Scenario: A window fills the frame
- **WHEN** one window is shown
- **THEN** it spans the frame's whole width and its mode line text starts 12 px from the left edge

### Requirement: Mode lines and the echo line share a height
Mode lines and the echo line SHALL be the line height plus 5 px, rounded, with their text centred in that height.

#### Scenario: Design size
- **WHEN** the font size is 15 and `line_height` is 1.4
- **THEN** mode lines and the echo line are 26 px high

### Requirement: A divider separates side-by-side windows
The 1 px gap between side-by-side windows SHALL be drawn in the `divider` face's background. Stacked windows SHALL have no divider, and the frame SHALL draw no other borders.

#### Scenario: Three windows
- **WHEN** a frame holds two windows side by side above a third
- **THEN** a 1 px `divider` stands between the two, down their full height with their mode lines, and none above the third

### Requirement: Ligatures are off
All text SHALL be shaped with contextual alternates off, with no setting, so `->` and `!=` draw as separate characters.

#### Scenario: A ligature font
- **WHEN** the font is Cascadia Code and a line holds `->`
- **THEN** `-` and `>` are drawn as two characters
