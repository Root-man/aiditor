# Spec Delta

## ADDED Requirements

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
When a file's left group is wider than its room, its directory SHALL lose its start to a `…`, and SHALL be left out when that is not enough, before the rest of the group is cut.

#### Scenario: A deep path in a narrow window
- **WHEN** `crates/core/src/display.rs` has room for all but three of its directory's columns
- **THEN** the left group starts `…es/core/src/display.rs`

#### Scenario: No room for the directory
- **WHEN** the room is under the file name's width plus one
- **THEN** the directory is gone and the name is cut only if it still doesn't fit

### Requirement: The major mode names the file's language
The major mode SHALL be the name of the language the file is highlighted in, and `Fundamental` for a file with no grammar.

#### Scenario: A Rust file
- **WHEN** `a.rs` is shown
- **THEN** the major mode is `Rust`

#### Scenario: No grammar
- **WHEN** `a.txt` is shown
- **THEN** the major mode is `Fundamental`

## MODIFIED Requirements

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line of two groups of segments, a segment appearing only when it applies. For a buffer not visiting a file, the left group SHALL hold whether it is read-only (`%%`), modified (`**`) or neither (`--`) with its name, then any agent mode, and the right group point's line (from 1) and column (from 0). For a buffer visiting a file, the left group SHALL hold `●` when modified and `⊘` when read-only, the file's directory relative to its project root, after that root's name in a project of several roots, and its name, then `[deleted]` when its file was deleted on disk or `[conflict]` when its unsaved edits conflict with a change to its file, then its major mode. The right group SHALL hold the active region's size, `UTF-8 CRLF` when its file uses CRLF line endings, its branch, point's line and column, then its position.

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

### Requirement: The frame display joins segments with separators
The frame display SHALL give each window its mode line's left and right groups as styled text, with ` │ ` between the segments of a group and none around a group or where a segment doesn't apply.

#### Scenario: Two segments
- **WHEN** a selected window shows a CRLF file
- **THEN** its right group reads `UTF-8 CRLF │ 1:0 │ All`, the `│` in the `separator` face

#### Scenario: A segment that doesn't apply
- **WHEN** a selected window shows an LF file
- **THEN** its right group reads `1:0 │ All`, with no separator before `1:0`

### Requirement: Mode line segments use colour only for state
In the selected window, `**` and `●` SHALL be in `status-modified`; `%%`, `⊘`, `[deleted]` and `[conflict]` in `status-failed`; `UTF-8 CRLF` and `[char]` in `status-waiting`; `[copy]` in `status-info`; the branch in `vcs-branch` and its ` ±` in `status-modified`; a file's directory, major mode, region size, position and `[unwatched]` in `mode-line-dim`; a file's name in `mode-line-buffer-id`. Other text SHALL be in the `mode-line` face, and every segment's face SHALL sit over it.

#### Scenario: A modified file in the selected window
- **WHEN** the selected window's buffer is modified
- **THEN** `●` has the `status-modified` foreground and the file's name is bold in the `mode-line` foreground
