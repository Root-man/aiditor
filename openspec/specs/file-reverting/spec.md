# file-reverting Specification

## Purpose
Reverting a buffer to its file on disk by applying a diff, so point, mark and markers stay with their text and the old text stays in undo history; used by `revert-buffer` and later by auto-revert and conflict resolution.

## Requirements

### Requirement: A revert applies the diff between buffer and file
A revert SHALL turn the buffer's text into the file's decoded text (as a visit decodes it: BOM stripped, CRLF read as LF) by changing only the lines that differ, computed off the core thread. Every marker, point and mark of each window showing the buffer included, SHALL follow those changes as it follows any edit, so one on an unchanged line stays on that line at the same column.

#### Scenario: Point stays on its line after lines above it change
- **WHEN** point is at column 1 of line `f` in a buffer of lines `a` to `j`, the file on disk replaces lines `b` and `c` with four new lines, and the buffer is reverted
- **THEN** the buffer holds the file's text and point is at column 1 of line `f`

#### Scenario: Mark and other windows keep their positions
- **WHEN** the mark is on line `h` and a second window shows the buffer at line `i`, and a revert removes line `a`
- **THEN** the mark is still on line `h` and the second window's point on line `i`

#### Scenario: Point in a changed line
- **WHEN** point is inside line `c` and the revert replaces line `c`
- **THEN** point is at the start of the replacement text

### Requirement: A revert is one undo group and leaves the buffer unmodified
The edits of one revert SHALL form one undo group, so one undo brings back the text from before it. Afterwards the buffer SHALL be unmodified and SHALL take the file's hash, line ending and BOM, so a save after the revert is not refused as a change on disk. A revert SHALL apply to a read-only buffer too.

#### Scenario: Undo brings the old text back
- **WHEN** a modified buffer is reverted and `C-/` is typed
- **THEN** the buffer holds its text from before the revert

#### Scenario: Unmodified with the file's hash
- **WHEN** the file changes on disk and the buffer is reverted
- **THEN** the buffer is unmodified, and an edit followed by `C-x C-s` writes the file

#### Scenario: Line ending follows the file
- **WHEN** an LF file is rewritten on disk with CRLF line endings and the buffer is reverted
- **THEN** the buffer holds LF text and the mode line shows `(DOS)`

### Requirement: A stale revert is dropped
A revert whose diff was computed against text the buffer no longer holds (the buffer was edited, or killed, while the diff was computed) SHALL change nothing.

#### Scenario: Edit while the diff is computed
- **WHEN** a revert is requested, a char is typed before its diff arrives, and then the diff arrives
- **THEN** the buffer keeps the typed char and stays modified

### Requirement: revert-buffer reverts the current buffer from its file
`revert-buffer` SHALL revert the current buffer from the file it visits. When the buffer is modified, it SHALL first ask `Revert buffer from file <path>? ` with `Yes` and `No`, and revert only on `Yes`; an unmodified buffer SHALL be reverted without asking. It SHALL fail, leaving the buffer as it was, with:
- `Buffer does not seem to be associated with any file` when the buffer visits no file;
- `File <path> no longer exists` when the file is gone;
- `<path>: binary file` or `<path>: not valid UTF-8` when the file could not be visited;
- the read's error when the file cannot be read.

#### Scenario: Unmodified buffer reverts at once
- **WHEN** the file changed on disk and `M-x revert-buffer RET` is typed in its unmodified buffer
- **THEN** the buffer holds the file's text without a prompt

#### Scenario: Modified buffer asks
- **WHEN** `M-x revert-buffer RET` is typed in a modified file buffer
- **THEN** the minibuffer asks `Revert buffer from file <path>? `, `No` leaves the buffer as it was, and `Yes` reverts it

#### Scenario: No file
- **WHEN** `M-x revert-buffer RET` is typed in `*scratch*`
- **THEN** the echo area shows `Buffer does not seem to be associated with any file`

#### Scenario: File deleted
- **WHEN** the visited file was deleted and `M-x revert-buffer RET` is typed
- **THEN** the echo area shows `File <path> no longer exists` and the buffer keeps its text

#### Scenario: File became binary
- **WHEN** the visited file now holds a NUL byte and `M-x revert-buffer RET` is typed
- **THEN** the echo area shows `<path>: binary file` and the buffer keeps its text
