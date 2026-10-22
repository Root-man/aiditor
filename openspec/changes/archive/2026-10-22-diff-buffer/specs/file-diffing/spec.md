# Spec Delta

## Purpose

Comparing a buffer's text with its file on disk as a unified diff in a read-only buffer, so a recovered or conflicted buffer's unsaved changes can be reviewed before saving or reverting.

## ADDED Requirements

### Requirement: diff-buffer-with-file shows the buffer against its file
`M-x diff-buffer-with-file` SHALL compare the current buffer's text with its file on disk, decoded as a visit decodes it, off the core thread. The result SHALL be a unified diff with the file as the old side (`--- <path>`) and the buffer as the new side (`+++ <buffer name>`), with three lines of context, held in a buffer named `*Diff*` of the current workspace. The command SHALL NOT change the compared buffer.

#### Scenario: A recovered buffer is compared with its file
- **WHEN** a recovered auto-save is accepted with `Yes` for a file whose disk text differs, and `M-x diff-buffer-with-file` is run
- **THEN** `*Diff*` holds a unified diff whose `-` lines are the disk's and `+` lines the recovered text's, and the recovered buffer is still modified with its text unchanged

#### Scenario: Unchanged lines are context
- **WHEN** a buffer of ten lines differs from its file in line 5 only
- **THEN** `*Diff*` shows one hunk with lines 2 to 4 and 6 to 8 as context

### Requirement: The diff buffer is read-only and reused
`*Diff*` SHALL be read-only and keep no undo history. Running the command again SHALL replace its text, keeping one `*Diff*` per workspace.

#### Scenario: Editing the diff fails
- **WHEN** a key that inserts text is typed in `*Diff*`
- **THEN** the echo area says `Buffer is read-only` and the text is unchanged

#### Scenario: A second run refills the same buffer
- **WHEN** the command runs twice with different edits in between
- **THEN** there is one `*Diff*` buffer and it holds only the second diff

### Requirement: The diff buffer is shown in another window
The command SHALL show `*Diff*` in a window other than the selected one, splitting the selected window when it is the only one, and SHALL leave the selected window and its buffer selected. A window already showing `*Diff*` SHALL be reused.

#### Scenario: A lone window splits
- **WHEN** the frame has one window and the command runs on a buffer that differs from its file
- **THEN** the frame has two windows, the selected one still shows the compared buffer, and the other shows `*Diff*`

#### Scenario: An existing window is reused
- **WHEN** `*Diff*` is already shown in another window and the command runs again
- **THEN** no window is added

### Requirement: Nothing to compare is reported
With no differences the command SHALL say `No differences` and SHALL NOT show `*Diff*`. A buffer visiting no file SHALL get `Buffer does not seem to be associated with any file`, a missing file `File <path> no longer exists`, and a file that can't be visited (binary, non-UTF-8, unreadable) `<path>: <reason>`.

#### Scenario: No differences
- **WHEN** the command runs on an unmodified buffer whose file is unchanged
- **THEN** the echo area says `No differences` and the frame still has one window

#### Scenario: No file, missing file, binary file
- **WHEN** the command runs on a buffer visiting no file, on one whose file was deleted, and on one whose file became binary
- **THEN** each says its message and no `*Diff*` is shown

### Requirement: A stale answer is dropped
A diff that arrives after its buffer was killed, or while another workspace is current, SHALL change nothing.

#### Scenario: Killed before the answer
- **WHEN** the buffer is killed before the diff arrives
- **THEN** no `*Diff*` buffer is made
