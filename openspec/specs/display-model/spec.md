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
The display SHALL end each window with a mode line of two groups of segments, a segment appearing only when it applies. For a terminal buffer, the left group SHALL hold its name, then the shell's directory, and the right group the running command, the last command's non-zero exit status, the input mode, then point's line and column in copy mode. For any other buffer not visiting a file, the left group SHALL hold whether it is read-only (`%%`), modified (`**`) or neither (`--`) with its name, then any agent mode, and the right group point's line (from 1) and column (from 0). For a buffer visiting a file, the left group SHALL hold `●` when modified and `⊘` when read-only, the file's directory relative to its project root, after that root's name in a project of several roots, and its name, then `[deleted]` when its file was deleted on disk or `[conflict]` when its unsaved edits conflict with a change to its file, then its major mode. The right group SHALL hold the active region's size, `UTF-8 CRLF` when its file uses CRLF line endings, its branch, point's line and column, then its position.

#### Scenario: A modified file
- **WHEN** `src/a.rs` under a project root has been edited and point is on line 3, column 2
- **THEN** the left group's segments are `● src/a.rs` and `Rust`, and the right group's `3:2` and the position

#### Scenario: A clean file at its root
- **WHEN** an unmodified `a.txt` at a project root is shown
- **THEN** the left group's segments are `a.txt` and `Fundamental`

#### Scenario: A file in a project of several roots
- **WHEN** `src/a.rs` under a root named `api` is shown in a project with two roots
- **THEN** the left group's first segment is `api/src/a.rs`

#### Scenario: A file outside the project's roots
- **WHEN** `/etc/hosts` is shown in a project with no root above it
- **THEN** the left group starts `/etc/hosts`

#### Scenario: A CRLF file
- **WHEN** an unmodified CRLF file `a.txt` that fits its window is shown with point at the start
- **THEN** the right group's segments are `UTF-8 CRLF`, `1:0` and `All`

#### Scenario: A read-only buffer
- **WHEN** a refused binary file `a.bin` is shown
- **THEN** the left group starts `⊘ a.bin`

#### Scenario: Read-only and modified
- **WHEN** a recovered read-only file is shown modified
- **THEN** the left group starts `● ⊘`

#### Scenario: A deleted file
- **WHEN** the file of an unmodified buffer `a.txt` is deleted on disk, with point at the start
- **THEN** the left group's segments are `a.txt`, `[deleted]` and `Fundamental`

#### Scenario: A conflicted file
- **WHEN** `x` is typed at the start of `a.txt` and another program then rewrites the file
- **THEN** the left group's segments are `● a.txt`, `[conflict]` and `Fundamental`

#### Scenario: A file on a branch
- **WHEN** a file in a repository on `main` is shown
- **THEN** the right group has `⎇ main` before `line:col`

#### Scenario: A file with a diff
- **WHEN** a file git reports as changed, on `main`, is shown
- **THEN** that segment reads `⎇ main ±`

#### Scenario: A buffer with no file
- **WHEN** `*scratch*` is shown modified with point on line 3, column 2
- **THEN** the left group is `** *scratch*` and the right group `3:2`

#### Scenario: A terminal
- **WHEN** terminal `t`'s shell is in `/srv` and a command is running
- **THEN** the left group's segments are `t` and `/srv`, and the right group's `◉ <command>` and `char`

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
The frame display SHALL carry workspace modeline spans for every workspace in creation order, each as `project › workspace`, separated by ` │ ` in `separator`. The current entry SHALL use the bright `workspace-bar` foreground with its workspace name in `workspace-current`; the other entries SHALL use `mode-line-dim`. Project names SHALL be bold and ` › ` SHALL use `separator`. Editing, splitting windows and prompting SHALL retain the workspace overview.

#### Scenario: Names follow the current workspace
- **GIVEN** workspaces `main` of `home` and then `w2` of `other`
- **WHEN** `w2` is renamed `w3` and `main` becomes current
- **THEN** the bar reads `home › main │ other › w3`, with `main` highlighted and the other entry dim

#### Scenario: Names stay while editing and prompting
- **GIVEN** a workspace overview
- **WHEN** the window is split, another buffer shown or the minibuffer opened
- **THEN** the workspace overview keeps its order and current highlight

### Requirement: The workspace modeline names are truncated to the frame width
The workspace modeline SHALL reserve five columns for the clock and two columns of separation. If all entries do not fit the remaining width at the workspace font size, it SHALL retain a contiguous range containing the current workspace and as many immediate neighbours as fit, expanding toward the nearest neighbours and preferring the preceding one on ties. Hidden entries SHALL fold into a trailing `+N` entry, separated by ` │ `, with the most urgent hidden activity dot. If the current entry itself is too wide, its names SHALL share the available room after separators and its dot: a name fitting half stays whole and the other takes the rest, otherwise each takes half with the workspace receiving the odd column. A cut name SHALL end in `…`. At widths insufficient for everything, retain the current names before the overflow entry and omit parts that cannot fit; the line SHALL stay within its budget.

#### Scenario: Overflow around a middle workspace
- **GIVEN** five short workspace entries and room for three entries plus overflow
- **WHEN** the middle workspace becomes current
- **THEN** the bar shows it and its immediate neighbours in creation order, then `+2` with the most urgent hidden dot

#### Scenario: A long current name
- **GIVEN** a current workspace name too long for the modeline
- **WHEN** the frame narrows
- **THEN** the current entry remains visible, its names shortened with `…`, and the bar fits the remaining width

#### Scenario: A wider workspace font
- **GIVEN** a frame with overflowing workspaces
- **WHEN** the workspace font size increases
- **THEN** fewer entries fit and the current entry stays visible

#### Scenario: Names that fit stay whole
- **GIVEN** project `aiditor` and workspace `watcher-fix`
- **WHEN** the entry has 40 columns after reserving the clock
- **THEN** both names stay whole

#### Scenario: A long workspace name is cut
- **GIVEN** project `Legacy crap` and a 60-character workspace name
- **WHEN** the current entry has 40 columns after reserving the clock
- **THEN** the project stays whole and the workspace is cut to 26 characters ending in `…`

#### Scenario: Two long names are both cut
- **GIVEN** two 30-character project and workspace names
- **WHEN** the current entry has 23 columns after reserving the clock
- **THEN** each name is cut to 10 characters ending in `…`

### Requirement: Text keeps clear of the window's sides
A window's text SHALL start 12 px from its left edge where it has no gutter, and SHALL stop at least 12 px before its right edge. The display SHALL give each window the columns that leave, and the frame display SHALL report where the text starts.

#### Scenario: No gutter
- **WHEN** a window 100 px wide with no gutter is shown in 8 px cells
- **THEN** its lines hold 9 columns and its text starts at 12 px

#### Scenario: The minibuffer
- **WHEN** a prompt is open in a frame 100 px wide with 8 px cells
- **THEN** the minibuffer's line scrolls in 9 columns less the prompt

### Requirement: The frame display joins segments with separators
The frame display SHALL give each window its mode line's left and right groups as styled text, with ` │ ` between the segments of a group and none around a group or where a segment doesn't apply.

#### Scenario: Two segments
- **WHEN** a selected window shows a CRLF file
- **THEN** its right group reads `UTF-8 CRLF │ 1:0 │ All`, the `│` in the `separator` face

#### Scenario: A segment that doesn't apply
- **WHEN** a selected window shows an LF file
- **THEN** its right group reads `1:0 │ All`, with no separator before `1:0`

### Requirement: Mode line segments use colour only for state
In the selected window, `**` and `●` SHALL be in `status-modified`; `%%`, `⊘`, `[deleted]` and `[conflict]` in `status-failed`; `UTF-8 CRLF` and `char` in `status-waiting`; `copy` in `status-info`; a running command in `status-running`; `exit <n>` in `status-failed`; the branch in `vcs-branch` and its ` ±` in `status-modified`; a file's or shell's directory, region size, position and `[unwatched]` in `mode-line-dim`; a file's name and a terminal's name in `mode-line-buffer-id`. A session model and effort SHALL use `mode-line-dim`, its title `mode-line-buffer-id`, context usage `mode-line-dim`, and mode and lifecycle status their specified risk and status faces. The file's language name and version SHALL use an overridable `mode-line-<language>` face: Elixir and HEEx purple/rose (`mode-line-elixir`), JavaScript green (`mode-line-javascript`), TypeScript and TSX blue (`mode-line-typescript`), Rust rusty red (`mode-line-rust`), YAML yellow (`mode-line-yaml`), Markdown aqua (`mode-line-markdown`), and TOML orange (`mode-line-toml`). Fundamental SHALL retain `mode-line-dim`. This language coding is an exception to the state-only colour rule. Other text SHALL be in the `mode-line` face, and every segment's face SHALL sit over it. Inactive windows SHALL retain their shared inactive face.

#### Scenario: A modified file in the selected window
- **GIVEN** the selected window shows a file
- **WHEN** its buffer is modified
- **THEN** `●` has the `status-modified` foreground and the file's name is bold in the `mode-line` foreground

#### Scenario: Every supported language has a color
- **GIVEN** files in every supported highlighted language, with or without a pin
- **WHEN** each file is shown in the selected window
- **THEN** its entire language segment uses its language family's face, including any displayed version

#### Scenario: Inactive language colors
- **GIVEN** a file with a colored language segment
- **WHEN** its window becomes inactive
- **THEN** the language name and version use the shared `mode-line-inactive` face

#### Scenario: Theme override
- **GIVEN** settings override a language face's foreground
- **WHEN** settings are reloaded
- **THEN** the file's language name and version use the new foreground

### Requirement: An inactive mode line drops colour
A window that isn't selected SHALL show the same groups, segments and separators as when selected, all of it in the `mode-line-inactive` face, with no colour or bold from any segment.

#### Scenario: A modified CRLF file in an inactive window
- **WHEN** a modified CRLF file is shown in a window that isn't selected
- **THEN** the mode line has the same text as in the selected window and every span has the `mode-line-inactive` face

### Requirement: A wide mode line is cut to its window
When both groups and a two-column gap don't fit between the window's 12 px insets, a group that fits in half the room SHALL stay whole and the other SHALL take the rest, else each SHALL take half, the right group getting any odd column. A cut group SHALL end in `…`, and a group with no room SHALL show nothing.

#### Scenario: A long name
- **WHEN** a buffer with a long name is shown in a narrow window with its right group `1:0`
- **THEN** the right group stays `1:0` and the left group is cut to the rest, ending in `…`

#### Scenario: Both groups too long
- **WHEN** both groups are wider than half the room
- **THEN** each is cut to half of it, ending in `…`

#### Scenario: It fits
- **WHEN** both groups and the gap fit
- **THEN** neither group is cut

### Requirement: A file's mode line shows its position
The position SHALL be `All` when the window shows the buffer's first and last text, `Top` when only its first, `Bot` when only its last, else `n%`, the chars above the window as a whole percentage of the buffer.

#### Scenario: A short file
- **WHEN** a file that fits its window is shown
- **THEN** the position is `All`

#### Scenario: The top of a long file
- **WHEN** a 100-line file is shown in a 10-line window from its first line
- **THEN** the position is `Top`

#### Scenario: The end of a long file
- **WHEN** point moves to the end of that file
- **THEN** the position is `Bot`

#### Scenario: The middle of a long file
- **WHEN** a 100-line file of equal lines is shown from line 41 in a 10-line window
- **THEN** the position is `40%`

#### Scenario: Wrapped rows run past the window
- **WHEN** a file's last line is the window's last but wraps below it
- **THEN** the position is not `Bot` or `All`

### Requirement: A file's mode line shows the region's size
While the region is active in a window showing a file, the right group SHALL start with the lines and chars it holds, as `3 lines, 84 chars`, each in the singular at one. A region ending at a line's start SHALL not count that line.

#### Scenario: A region over lines
- **WHEN** the region runs from the start of line 1 to column 2 of line 3, 14 chars in all
- **THEN** the right group starts `3 lines, 14 chars`

#### Scenario: Whole lines
- **WHEN** the region runs from the start of line 1 to the start of line 3
- **THEN** it counts 2 lines

#### Scenario: One line
- **WHEN** the region is one char
- **THEN** the right group starts `1 line, 1 char`

#### Scenario: No active region
- **WHEN** the mark is set but the region is not active
- **THEN** the right group has no size

### Requirement: A file's directory gives way before its name
When a file's or terminal's left group is wider than its room, its directory SHALL lose its start to a `…`, and SHALL be left out when that is not enough, before the rest of the group is cut.

#### Scenario: A deep path in a narrow window
- **WHEN** `crates/core/src/display.rs` has room for all but three of its directory's columns
- **THEN** the left group starts `…es/core/src/display.rs`

#### Scenario: No room for the directory
- **WHEN** the room is under the file name's width plus one
- **THEN** the directory is gone and the name is cut only if it still doesn't fit

### Requirement: The major mode names the file's language
The major mode SHALL be the name of the language the file is highlighted in, and `Fundamental` for a file with no grammar. For Rust, Elixir and HEEx, and JavaScript, TypeScript and TSX, it SHALL append the declared Rust, Elixir or Node.js version respectively when found above the file. Search SHALL start in the file's directory and ascend to the filesystem root, checking `.tool-versions`, `mise.toml`, then `rust-toolchain.toml` at each level; the first entry for the relevant tool SHALL win. An unrelated tool or unreadable/malformed file SHALL allow searching to continue. With no usable version, the segment SHALL show the language alone. Only file contents SHALL be read: no compiler, version manager, template or command SHALL be executed to resolve a version.

#### Scenario: A Rust file
- **GIVEN** `a.rs` has no pin above it
- **WHEN** it is shown
- **THEN** the major mode is `Rust`

#### Scenario: No grammar
- **GIVEN** `a.txt` has no grammar
- **WHEN** it is shown
- **THEN** the major mode is `Fundamental`

#### Scenario: Supported pin formats
- **GIVEN** a Rust or Elixir file below `.tool-versions` or `mise.toml` declaring its tool version, or a Rust file below `rust-toolchain.toml` declaring `toolchain.channel`
- **WHEN** it is shown
- **THEN** the language segment includes the declared version, such as `Rust 1.99.0` or `Elixir 1.20.2-otp-26`

#### Scenario: JavaScript runtime
- **GIVEN** JavaScript, TypeScript or TSX files below a `nodejs` entry in `.tool-versions` or a `node`/`nodejs` entry in `mise.toml`
- **WHEN** they are shown
- **THEN** their language names are followed by that runtime version

#### Scenario: Nested pins and ties
- **GIVEN** a pin at a parent directory and a nearer pin for the same tool
- **WHEN** a file under the nearer pin is shown
- **THEN** the nearer pin wins, with ties in one directory resolved in the filename order above

#### Scenario: Multiple versions and system selection
- **GIVEN** an entry declares several versions or selects `system`
- **WHEN** its file is shown
- **THEN** only the first version is used, and `system` shows the language alone without falling back to a parent pin

#### Scenario: Unsupported or invalid value
- **GIVEN** a pin value is empty, contains whitespace/control characters, or is a dynamic mise template
- **WHEN** its file is shown
- **THEN** that entry supplies no displayed version

### Requirement: Workspace dots aggregate agent session activity
Each workspace SHALL show a `●` after its name when any owned session is waiting for permission, failed to start or exited, or working on a prompt. Priority across sessions SHALL be waiting in `status-waiting`, then failed/exited in `status-failed`, then working in `status-running`. Live idle and starting sessions SHALL have no dot. The dot SHALL follow activity even in other workspaces; killing a session or workspace SHALL remove its contribution. Hidden dots SHALL aggregate with the same priority.

#### Scenario: Mixed activity across sessions
- **GIVEN** a workspace with working, failed and permission-waiting sessions
- **WHEN** the bar is built
- **THEN** its dot uses `status-waiting`
- **WHEN** the waiting session becomes idle
- **THEN** its dot uses `status-failed`
- **WHEN** the failed session is killed
- **THEN** its dot uses `status-running`

#### Scenario: Idle after completing a prompt
- **GIVEN** a working session in another workspace
- **WHEN** it completes its prompt with no pending permission
- **THEN** that workspace returns to having no dot

### Requirement: Language versions follow disk changes
Open file buffers in every workspace SHALL refresh their language versions when a candidate pin file changes, is created, deleted or atomically replaced, or when the watcher requests a rescan. This SHALL work with project watching disabled and outside project roots, including creation of a nearer pin. Replies from a superseded lookup or for a killed buffer SHALL be ignored. The version SHALL use the language segment's existing faces and fitting rules.

#### Scenario: Editing a shared pin
- **GIVEN** two workspaces have open files using the same pin
- **WHEN** another program edits or atomically replaces that pin
- **THEN** both buffers' language segments follow the new version without reopening files

#### Scenario: A nearer pin appears and disappears
- **GIVEN** an open file inherits a parent pin and project watching is disabled
- **WHEN** a nearer pin is created and then deleted
- **THEN** the segment first uses the nearer version and then returns to the parent's version

#### Scenario: Lost events or late replies
- **GIVEN** a version lookup is in flight
- **WHEN** a pin change or rescan starts a newer lookup and the older reply arrives last
- **THEN** the newer version remains displayed

#### Scenario: Inactive and narrow windows
- **GIVEN** a file with a pinned version
- **WHEN** its window is inactive or too narrow for its whole mode line
- **THEN** the version is dimmed with the rest of the inactive mode line or truncated under the existing group fitting rules
