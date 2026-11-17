## MODIFIED Requirements

### Requirement: revert-buffer reverts the current buffer from its file
`revert-buffer` SHALL revert the current buffer from the file it visits. When the buffer is modified, it SHALL first ask `Revert buffer from file <path>? ` with `No` and `Yes`, `No` first so `RET` keeps the buffer, and revert only on `Yes`; an unmodified buffer SHALL be reverted without asking. It SHALL fail, leaving the buffer as it was, with:
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

#### Scenario: RET keeps the edits
- **WHEN** `M-x revert-buffer RET RET` is typed in a modified file buffer
- **THEN** the buffer keeps its edits and stays modified

#### Scenario: No file
- **WHEN** `M-x revert-buffer RET` is typed in `*scratch*`
- **THEN** the echo area shows `Buffer does not seem to be associated with any file`

#### Scenario: File deleted
- **WHEN** the visited file was deleted and `M-x revert-buffer RET` is typed
- **THEN** the echo area shows `File <path> no longer exists` and the buffer keeps its text

#### Scenario: File became binary
- **WHEN** the visited file now holds a NUL byte and `M-x revert-buffer RET` is typed
- **THEN** the echo area shows `<path>: binary file` and the buffer keeps its text
