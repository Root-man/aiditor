# display-model Specification

## Purpose
The display model is what the core shows for a window, built from core state for the frame size, so the frontend only draws it and the display is testable without a frontend.

## Requirements

### Requirement: Visible lines fill the window
Given the window's height in pixels, the display SHALL list the lines shown from the window's first line down, each with its buffer line number and height, as many as fit above the mode line, plus one the mode line cuts off when they don't fill the height. Every line SHALL be one line height (the font size times `line_height`) while the font is monospace. Point's line SHALL be among the rows that fit whole.

#### Scenario: A short buffer
- **WHEN** a three-line buffer is shown in a window ten lines high
- **THEN** the display has lines 0, 1 and 2

#### Scenario: A long buffer scrolled to point
- **WHEN** a window fits three lines plus the mode line and point is on line 5
- **THEN** the display shows lines 3, 4 and 5

#### Scenario: A partly visible last row
- **WHEN** a five-line buffer is shown in a window 2.5 lines high above the mode line and point moves to line 2
- **THEN** the display shows lines 1, 2 and 3, point's line among the two that fit whole

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
When the mark is set and active, the text between point and mark SHALL be drawn in the theme's `region` face, over any other face.

#### Scenario: Region across a face
- **WHEN** the mark is active at 0 and point at 3 in `hello`
- **THEN** `hel` has the `region` background and `lo` the default face

#### Scenario: No mark
- **WHEN** the mark is unset
- **THEN** no span has the `region` background

#### Scenario: Inactive mark
- **WHEN** the mark is at 0 and inactive and point at 3 in `hello`
- **THEN** no span has the `region` background

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line showing whether the buffer is read-only (`%%`), modified (`**`) or neither (`--`), its name followed by ` (DOS)` when its file uses CRLF line endings, ` [deleted]` when its file was deleted on disk and ` [conflict]` when its unsaved edits conflict with a change to its file, and point's line (from 1) and column (from 0).

#### Scenario: A modified file
- **WHEN** `a.txt` has been edited and point is on line 3, column 2
- **THEN** the mode line reads `** a.txt  3:2`

#### Scenario: A CRLF file
- **WHEN** an unmodified CRLF file `a.txt` is shown with point at the start
- **THEN** the mode line reads `-- a.txt (DOS)  1:0`

#### Scenario: A read-only buffer
- **WHEN** a refused binary file `a.bin` is shown
- **THEN** the mode line reads `%% a.bin  1:0`

#### Scenario: A deleted file
- **WHEN** the file of an unmodified buffer `a.txt` is deleted on disk, with point at the start
- **THEN** the mode line reads `-- a.txt [deleted]  1:0`

#### Scenario: A conflicted file
- **WHEN** `x` is typed at the start of `a.txt` and another program then rewrites the file
- **THEN** the mode line reads `** a.txt [conflict]  1:1`

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

### Requirement: The frame display carries the echo area message
The frame display SHALL include the echo area's text: the current message, or none when the echo area is blank.

#### Scenario: A command error in a frame snapshot
- **WHEN** a command fails with `boom`
- **THEN** the frame display's message is `boom`

#### Scenario: No message
- **WHEN** a buffer is shown and nothing has been reported
- **THEN** the frame display has no message

### Requirement: Point's line scrolls sideways to keep point visible
In a buffer that truncates lines, given the window's width in columns, the display SHALL show point's display line from the window's horizontal scroll offset on, the cursor column counted from it. The offset SHALL be 0 while point's column fits the window, and otherwise SHALL move just far enough that the cursor is within the window's columns. Other lines SHALL be shown from column 0, truncated at the window edge. Each display line SHALL hold at most the window's columns plus one, the last one for a partly visible column; a display with no width SHALL cut nothing. A column is one char. A buffer that wraps SHALL have an offset of 0.

#### Scenario: End of a long line
- **WHEN** point moves with `C-e` to the end of a 100-char line in a window 40 columns wide, in a buffer that truncates lines
- **THEN** that line is shown from column 61 with the cursor at column 39, and the window's other lines start at column 0

#### Scenario: Back to a column that fits
- **WHEN** point then moves with `C-a` to the start of the line
- **THEN** the line is shown from column 0 with the cursor at column 0

#### Scenario: A line far wider than the window
- **WHEN** a buffer that truncates lines holds a 10 000-char line and the next line, and point is at the end of the long line in a window 20 columns wide
- **THEN** the long line holds its last 19 chars, from column 9 981, with the cursor at column 19, and with point on the next line the long line holds its first 21 chars

#### Scenario: The minibuffer
- **WHEN** the minibuffer's text is wider than the frame less its prompt
- **THEN** its line scrolls the same way to keep the cursor visible

### Requirement: The frame title names the buffer, workspace and project
The frame display's title SHALL be the current buffer's name, followed by ` *` when that buffer is modified, then ` | `, the current workspace's name, ` | ` and the name of the project that workspace is bound to. The frontend SHALL show it as the window title.

#### Scenario: Title of an unmodified buffer
- **WHEN** the current workspace is `main`, bound to project `aiditor`, and shows unmodified `t.txt`
- **THEN** the title is `t.txt | main | aiditor`

#### Scenario: Title of a modified buffer
- **WHEN** `t.txt` is modified in that workspace
- **THEN** the title is `t.txt * | main | aiditor`

#### Scenario: Title follows the current workspace
- **WHEN** a second workspace `w2` is made for project `other` and becomes current
- **THEN** the title is `*scratch*<w2> | w2 | other`
- **WHEN** it is renamed to `w3`
- **THEN** the title is `*scratch*<w3> | w3 | other`
- **WHEN** the first workspace is made current again
- **THEN** the title names `main` and `aiditor` again

### Requirement: Long lines wrap into visual rows
Unless its buffer truncates lines, a window at least 2 columns wide SHALL show each buffer line as visual rows of at most its width less one char, each a display line carrying the buffer line's number and decorations. A session buffer and a buffer visiting a `.md`, `.markdown` or `.txt` file SHALL break a row after the last whitespace that fits, a whitespace char also fitting in the last column, and at the width when no whitespace fits. Every other buffer SHALL break at the width and end each row but a line's last with a `\` continuation marker in the last column, in the theme's `comment` syntax face. The window's first row MAY be a row inside its first buffer line. The cursor SHALL be on the row holding point, at point's column from the row's start; point at a break SHALL be at the start of the next row. Only the chars of the rows the window can show SHALL be laid out: the rows of the first buffer line above the window's first row SHALL NOT be.

#### Scenario: Code wraps at the edge
- **WHEN** `a.rs` holds a 10-char line `abcdefghij` in a window 5 columns wide
- **THEN** the display lines read `abcd\`, `efgh\` and `ij`, all buffer line 0

#### Scenario: Markdown wraps at words
- **WHEN** `a.md` holds `one two three` in a window 9 columns wide
- **THEN** the display lines read `one two ` and `three`

#### Scenario: A word longer than the window
- **WHEN** `a.md` holds `abcdefghij` in a window 5 columns wide
- **THEN** the display lines read `abcd`, `efgh` and `ij`

#### Scenario: The cursor on a wrapped line
- **WHEN** point is at column 5 of `abcdefghij` in `a.rs` in a window 5 columns wide
- **THEN** the cursor is on the second row at column 1

#### Scenario: A short line
- **WHEN** a line fits in the window less one column
- **THEN** it is one display line without a marker

#### Scenario: The window starts deep inside a line
- **WHEN** `a.md` holds 1,000 copies of `word ` on one line, a window 21 columns wide shows 3 rows and point is at the line's end
- **THEN** the display lines are the line's last rows as wrapping the whole line at words would break them, the cursor is on the last row after its final char, and laying out the display reads none of the line's chars before the first row shown

### Requirement: toggle-truncate-lines switches a buffer between wrapping and truncation
`toggle-truncate-lines` SHALL switch the current buffer between wrapping and truncating long lines and echo `Truncate long lines enabled` or `Truncate long lines disabled`. Buffers SHALL wrap until toggled. The minibuffer SHALL always truncate.

#### Scenario: Toggling to truncation and back
- **WHEN** `M-x toggle-truncate-lines` runs in a buffer with a line wider than its window
- **THEN** the echo area says `Truncate long lines enabled` and the line is one display line cut at the window edge
- **WHEN** it runs again
- **THEN** the echo area says `Truncate long lines disabled` and the line wraps again

### Requirement: The frame display names the current project and workspace
The frame display SHALL carry the name of the current workspace's project and the current workspace's name for the workspace modeline, whatever buffer, window or minibuffer is current.

#### Scenario: Names follow the current workspace
- **WHEN** the current workspace is `main`, bound to project `home`
- **THEN** the frame display names project `home` and workspace `main`
- **WHEN** a workspace `w2` for project `other` becomes current and is renamed `w3`
- **THEN** it names project `other` and workspace `w3`
- **WHEN** `main` is made current again
- **THEN** it names `home` and `main` again

#### Scenario: Names stay while editing and prompting
- **WHEN** the window is split, another window selected, another buffer shown or the minibuffer opened
- **THEN** the frame display still names the current project and workspace

### Requirement: The workspace modeline names are truncated to the frame width
When `<project> › <workspace>` is wider than the frame's columns, the names SHALL be cut so the line fits: a name that fits in half the room left after ` › ` stays whole and the other takes the rest, otherwise each takes half, the workspace getting the odd column. A cut name ends in `…` and keeps at least that character when it has any room.

#### Scenario: Names that fit stay whole
- **WHEN** project `aiditor` and workspace `watcher-fix` are shown 40 columns wide
- **THEN** the names are `aiditor` and `watcher-fix`

#### Scenario: A long workspace name is cut
- **WHEN** project `Legacy crap` and a 60-character workspace name are shown 40 columns wide
- **THEN** the project stays `Legacy crap` and the workspace is cut to 26 characters ending in `…`

#### Scenario: Two long names are both cut
- **WHEN** two 30-character names are shown 23 columns wide
- **THEN** each is cut to 10 characters ending in `…`

### Requirement: Text keeps clear of the window's sides
A window's text SHALL start 12 px from its left edge where it has no gutter, and SHALL stop at least 12 px before its right edge. The display SHALL give each window the columns that leave, and the frame display SHALL report where the text starts.

#### Scenario: No gutter
- **WHEN** a window 100 px wide with no gutter is shown in 8 px cells
- **THEN** its lines hold 9 columns and its text starts at 12 px

#### Scenario: The minibuffer
- **WHEN** a prompt is open in a frame 100 px wide with 8 px cells
- **THEN** the minibuffer's line scrolls in 9 columns less the prompt
