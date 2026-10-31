# Spec Delta

## MODIFIED Requirements

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line of two groups of segments, a segment appearing only when it applies. The left group SHALL hold whether the buffer is read-only (`%%`), modified (`**`) or neither (`--`) with its name, then `[deleted]` when its file was deleted on disk or `[conflict]` when its unsaved edits conflict with a change to its file, then any agent mode. The right group SHALL hold `(DOS)` when its file uses CRLF line endings, then point's line (from 1) and column (from 0).

#### Scenario: A modified file
- **WHEN** `a.txt` has been edited and point is on line 3, column 2
- **THEN** the left group is `** a.txt` and the right group `3:2`

#### Scenario: A CRLF file
- **WHEN** an unmodified CRLF file `a.txt` is shown with point at the start
- **THEN** the left group is `-- a.txt` and the right group's segments are `(DOS)` and `1:0`

#### Scenario: A read-only buffer
- **WHEN** a refused binary file `a.bin` is shown
- **THEN** the left group is `%% a.bin` and the right group `1:0`

#### Scenario: A deleted file
- **WHEN** the file of an unmodified buffer `a.txt` is deleted on disk, with point at the start
- **THEN** the left group's segments are `-- a.txt` and `[deleted]`, and the right group is `1:0`

#### Scenario: A conflicted file
- **WHEN** `x` is typed at the start of `a.txt` and another program then rewrites the file
- **THEN** the left group's segments are `** a.txt` and `[conflict]`, and the right group is `1:1`

## ADDED Requirements

### Requirement: The frame display joins segments with separators
The frame display SHALL give each window its mode line's left and right groups as styled text, with `  │  ` between the segments of a group and none around a group or where a segment doesn't apply.

#### Scenario: Two segments
- **WHEN** a selected window shows a CRLF file
- **THEN** its right group reads `(DOS)  │  1:0`, the `│` in the `separator` face

#### Scenario: A segment that doesn't apply
- **WHEN** a selected window shows an LF file
- **THEN** its right group reads `1:0`, with no separator

### Requirement: Mode line segments use colour only for state
In the selected window, `**` SHALL be in `status-modified`; `%%`, `[deleted]` and `[conflict]` in `status-failed`; `(DOS)` and `[char]` in `status-waiting`; `[copy]` in `status-info`; `[unwatched]` in `mode-line-dim`. Other text SHALL be in the `mode-line` face, and every segment's face SHALL sit over it.

#### Scenario: A modified file in the selected window
- **WHEN** the selected window's buffer is modified
- **THEN** `**` has the `status-modified` foreground and the name the `mode-line` foreground

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
