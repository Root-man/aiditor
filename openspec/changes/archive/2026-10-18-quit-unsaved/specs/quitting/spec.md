# Spec Delta

## Purpose

Quitting the app without losing unsaved edits in file buffers.

## ADDED Requirements

### Requirement: Quitting offers to save modified file buffers
`save-buffers-kill-terminal` (`C-x C-c`, `s-q`) SHALL ask `Save file <path>? ` for each modified buffer that visits a file, one at a time, completing over `Yes` then `No`, matched case-insensitively. Only `Yes` SHALL save that buffer. Buffers that visit no file SHALL NOT be asked about. With no modified file buffer it SHALL quit at once.

#### Scenario: Nothing modified
- **WHEN** `C-x C-c` is typed with no modified file buffer
- **THEN** the app quits

#### Scenario: Saving then quitting
- **WHEN** a file buffer is modified and `C-x C-c RET` is typed
- **THEN** the prompt was `Save file <path>? `, the file is written and the app quits

#### Scenario: Scratch is not asked about
- **WHEN** only `*scratch*` is modified and `C-x C-c` is typed
- **THEN** the app quits without a prompt

### Requirement: Declining keeps the editor open unless confirmed
When any buffer was declined, the command SHALL ask `Modified buffers exist; exit anyway? `, completing over `No` then `Yes`. Only `Yes` SHALL quit; any other answer SHALL keep the editor open with every buffer as it was, apart from the saves already chosen.

#### Scenario: Declining stays open
- **WHEN** a file buffer is modified and `C-x C-c no RET RET` is typed
- **THEN** the app does not quit and the buffer is still modified

#### Scenario: Exit anyway
- **WHEN** a file buffer is modified and `C-x C-c no RET yes RET` is typed
- **THEN** the app quits and the file is unchanged on disk

### Requirement: Quitting waits for the saves
The app SHALL quit only after every save the command started has succeeded and left its buffer unmodified. A failed save SHALL cancel the quit, its error shown in the echo area.

#### Scenario: A save fails
- **WHEN** a file buffer is modified, the file changes on disk, and `C-x C-c RET` is typed
- **THEN** the echo area shows `<path> changed on disk; not saved` and the app does not quit

### Requirement: Closing the window quits the same way
A request to close the window SHALL NOT close it; it SHALL run `save-buffers-kill-terminal` instead.

#### Scenario: Close with a modified buffer
- **WHEN** a file buffer is modified and the window is asked to close
- **THEN** the window stays open and the minibuffer asks `Save file <path>? `
