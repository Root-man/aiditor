# Spec Delta

## ADDED Requirements

### Requirement: Writing input
Bytes written to a terminal SHALL reach its PTY unchanged and in the order they were written. Writing SHALL return to the caller at once, without waiting for the PTY to accept the bytes, so a write larger than the PTY's input buffer to a program that doesn't read never blocks the UI thread. The PTY's line discipline SHALL act on the bytes as on a real terminal's input: echo, line editing and signal characters. A write to an unknown terminal, or to one whose process has gone, SHALL be ignored.

#### Scenario: Input echoes back
- **WHEN** a terminal runs `cat` and `hi\n` is written to it, followed by `\x04`
- **THEN** its output holds `hi` twice, the echo and `cat`'s copy, then `cat` exits with code 0

#### Scenario: Ctrl-C interrupts the foreground job
- **WHEN** a terminal runs `sleep 30` and `\x03` is written to it
- **THEN** the exit message arrives within seconds, and its status is not a clean exit

#### Scenario: A large write to a program that doesn't read
- **WHEN** a terminal runs a program in raw mode that never reads, and 1 MB is written to it
- **THEN** the write returns at once

### Requirement: Resizing
A terminal SHALL be resizable to a number of rows and columns, setting its PTY's window size, so programs in it see the new size and the foreground job gets `SIGWINCH`. A resize SHALL take effect before any bytes written after it are read by the program. A resize of an unknown terminal SHALL be ignored.

#### Scenario: stty sees the new size
- **WHEN** a terminal spawned at 24 by 80 runs `read x; stty size`, is resized to 30 rows and 100 columns, and then gets `\n`
- **THEN** its output holds `30 100`

## MODIFIED Requirements

### Requirement: UTF-8 line editing
The terminal's PTY SHALL have the `IUTF8` input flag set on macOS and Linux, so the kernel's canonical-mode line editing treats a multibyte UTF-8 char as one char.

#### Scenario: The flag is on
- **WHEN** a terminal runs `stty -a`
- **THEN** its output lists `iutf8` without a leading `-`

#### Scenario: Backspace deletes a whole multibyte char
- **WHEN** a terminal in canonical mode runs `read -r line; printf '<%s>' "$line"` and gets `aé`, the erase char `\x7f` and `\n`
- **THEN** its output holds `<a>`
