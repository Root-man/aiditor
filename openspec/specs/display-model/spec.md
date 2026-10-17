# display-model Specification

## Purpose
The display model is what the core shows for a window, built from core state for the frame size, so the frontend only draws it and the display is testable without a frontend.

## Requirements

### Requirement: Visible lines fill the window
Given the window's height in pixels, the display SHALL list the lines shown from the window's first line down, each with its buffer line number and height, as many as fit above the mode line. Every line SHALL be one line height (the font size times 1.4) while the font is monospace. Point's line SHALL be among them.

#### Scenario: A short buffer
- **WHEN** a three-line buffer is shown in a window ten lines high
- **THEN** the display has lines 0, 1 and 2

#### Scenario: A long buffer scrolled to point
- **WHEN** a window fits three lines plus the mode line and point is on line 5
- **THEN** the display shows lines 3, 4 and 5

### Requirement: Lines are runs of styled spans
Each display line SHALL hold its text, without the LF, as spans: runs of chars with one face, where a face is an optional foreground and background color. Chars no face property covers SHALL have the default face. Where face properties overlap, the later-added one SHALL win for each color it sets.

#### Scenario: A face in the middle of a line
- **WHEN** `hello world` has a face with a red foreground on 6..11
- **THEN** its line is a default-face span `hello ` then a red span `world`

### Requirement: Invisible text folds into its placeholder
Text with the `invisible` property SHALL not be shown; its placeholder SHALL appear once in its place as a span marked as a fold. An invisible range that contains LFs SHALL join the lines it covers into one display line. An empty placeholder SHALL hide the text without a span.

#### Scenario: A folded block
- **WHEN** 3..7 of `a\nb\nc\nd\ne` (the LF after `b` through `d`) is invisible with placeholder `…`
- **THEN** the display lines read `a`, `b…` and `e`, the last one being buffer line 4

#### Scenario: Hidden markup
- **WHEN** the `**` around `**bold**` are invisible with an empty placeholder
- **THEN** the line reads `bold`

### Requirement: Lines carry their decorations
A line decoration property SHALL apply to every display line its range touches. A decoration SHALL be a background color for the whole line or a rule beside it in a color.

#### Scenario: A code block background
- **WHEN** lines 1 and 2 have a background decoration
- **THEN** those display lines carry it and lines 0 and 3 do not

### Requirement: The cursor is placed on its display line
The display SHALL give the cursor as a display line and a char column in that line's shown text. Point inside invisible text SHALL put the cursor at the start of its placeholder.

#### Scenario: Cursor after a fold
- **WHEN** point is right after a fold with placeholder `…` that hides `abc` at the start of the line
- **THEN** the cursor is at column 1

### Requirement: The region is highlighted
When the mark is set, the text between point and mark SHALL have the theme's region color as its background, over any face's background.

#### Scenario: Region across a face
- **WHEN** the mark is at 0 and point at 3 in `hello`
- **THEN** `hel` has the region background and `lo` the default face

#### Scenario: No mark
- **WHEN** the mark is unset
- **THEN** no span has the region background

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line showing whether the buffer is modified (`**` or `--`), its name, and point's line (from 1) and column (from 0).

#### Scenario: A modified file
- **WHEN** `a.txt` has been edited and point is on line 3, column 2
- **THEN** the mode line reads `** a.txt  3:2`
