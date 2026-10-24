# terminal-buffers Specification

## Purpose
Holds a terminal emulator's grid in a buffer, fed by the terminal's output, with the emulator's replies sent back to the terminal.

## Requirements

### Requirement: A terminal buffer holds the emulator's screen
A terminal buffer SHALL hold a VT emulator grid of the rows and columns it was created with. The terminal's output, fed in order, SHALL leave the grid's screen as a real xterm-compatible terminal would show it, cursor moves, erases, line wraps and scrolling included. Lines that scroll off the top SHALL go to the scrollback, not be lost.

#### Scenario: Plain lines
- **WHEN** `one\r\ntwo\r\n` is fed into a 4x10 terminal buffer
- **THEN** its screen reads `one`, `two` and two empty lines

#### Scenario: Cursor moves and clears
- **WHEN** `hello\x1b[1;1Hj` is fed, then `\x1b[2J\x1b[Hnew`
- **THEN** the screen first reads `jello`, then only `new`

#### Scenario: Scrolling
- **WHEN** five lines `1` to `5` are fed into a 3-row terminal buffer
- **THEN** the screen reads `3`, `4` and `5`, and `1` and `2` are in the scrollback

#### Scenario: Output split across batches
- **WHEN** an escape sequence is split between two output batches
- **THEN** the screen is the same as if it arrived in one

### Requirement: The emulator's replies go back to the terminal
Replies the emulator writes back to the program, such as cursor position and device attribute reports, SHALL be sent as input to that terminal, in order, after the output batch that asked for them. Title changes, the bell and clipboard requests SHALL be ignored.

#### Scenario: Cursor position report
- **WHEN** `ab\x1b[6n` is fed into a terminal buffer
- **THEN** the core asks the terminal service to write `\x1b[1;3R` to that terminal

#### Scenario: Device attributes
- **WHEN** `\x1b[c` is fed into a terminal buffer
- **THEN** the core writes a primary device attributes reply to that terminal

#### Scenario: Ignored requests
- **WHEN** a title change, a bell and an OSC 52 clipboard store are fed into a terminal buffer
- **THEN** nothing is written back and the clipboard is untouched

### Requirement: Output for an unknown terminal is ignored
Output for a terminal id the core has no live terminal buffer for SHALL change nothing and send nothing.

#### Scenario: Unknown id
- **WHEN** output arrives for a terminal id the core never created
- **THEN** no buffer changes and no effect is emitted

### Requirement: A terminal buffer is read-only text
A terminal buffer's text SHALL be empty and the buffer read-only, so text commands run in it without panicking: motion, `mark-whole-buffer` and line search find nothing to move over, and edits fail with `Buffer is read-only` as in any read-only buffer.

#### Scenario: Rope commands
- **WHEN** a terminal buffer is current and `C-f C-n M-> C-x h` and a line search run in it
- **THEN** none panics and point stays at 0

#### Scenario: Edits
- **WHEN** a char is typed, or `C-y` yanks, in a terminal buffer
- **THEN** the echo area says `Buffer is read-only` and its grid is unchanged
