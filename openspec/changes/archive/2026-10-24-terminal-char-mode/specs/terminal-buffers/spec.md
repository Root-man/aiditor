## ADDED Requirements

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live and no key sequence is pending, every key except `C-x`, `C-c`, `M-x` and keys with super held SHALL be written to the shell, encoded as xterm does, instead of running a command. Those four and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

#### Scenario: Typing a command
- **WHEN** `echo hi` is typed and `RET` pressed in a terminal
- **THEN** the shell receives `echo hi\r`

#### Scenario: Readline keys reach the shell
- **WHEN** `C-a C-e C-r M-b C-g C-u` are pressed in a terminal
- **THEN** the shell receives `\x01\x05\x12\x1bb\x07\x15` and no command runs

#### Scenario: Editor keys still work
- **WHEN** `C-x b`, `M-x` or a `C-c` sequence is pressed in a terminal
- **THEN** its command runs and the shell receives nothing

#### Scenario: Keys go to the minibuffer while it is open
- **WHEN** `M-x` is pressed in a terminal and `vterm` typed
- **THEN** the text goes to the minibuffer, not the shell

### Requirement: Keys are encoded as xterm does
Chars SHALL be sent as UTF-8; `C-` with a letter or one of `@[\]^_` as its control byte (shift ignored), `C-SPC` as `\x00` and `C-?` as `\x7f`; `M-` as an ESC before the key's encoding; `RET` as `\r` (`S-RET` `\n`), `TAB` `\t`, `S-TAB` `\x1b[Z`, `ESC` `\x1b`, `DEL` `\x7f` (`C-DEL` `\x08`). Unmodified arrows, Home and End SHALL send `\x1b[X` in normal cursor mode and `\x1bOX` in application cursor mode, as the program set with DECCKM; with modifiers, `\x1b[1;<m>X`, where `<m>` is 1 plus 1 for shift, 2 for meta and 4 for ctrl. PageUp, PageDown and Delete SHALL send `\x1b[5~`, `\x1b[6~` and `\x1b[3~`; F1–F4 `\x1bOP`–`\x1bOS`; F5–F12 `\x1b[<n>~` with n 15, 17–21, 23, 24; modified, `\x1b[<n>;<m>~` and `\x1b[1;<m>P`–`S`.

#### Scenario: Cursor modes
- **WHEN** `<up>` is pressed before and after the program sends `\x1b[?1h`
- **THEN** the shell receives `\x1b[A`, then `\x1bOA`

#### Scenario: Modified arrows
- **WHEN** `C-<left>` is pressed in either cursor mode
- **THEN** the shell receives `\x1b[1;5D`

### Requirement: C-c C-c interrupts
`C-c C-c` in a terminal buffer SHALL write `\x03` to its shell.

#### Scenario: Interrupt
- **WHEN** `C-c C-c` is pressed in a terminal
- **THEN** the shell receives `\x03`

### Requirement: Typed text goes to the shell
Text from the platform's text input, including input-method commits, SHALL be written to a live terminal's shell as UTF-8 when no key sequence is pending; a range the input method asks to replace SHALL be ignored.

#### Scenario: Non-ASCII text
- **WHEN** `é` arrives as typed text in a terminal
- **THEN** the shell receives its UTF-8 bytes

## MODIFIED Requirements

### Requirement: A terminal buffer is read-only text
A terminal buffer's text SHALL be empty and the buffer read-only, so text commands run in it by name without panicking: motion, `mark-whole-buffer` and line search find nothing to move over, and edits fail with `Buffer is read-only` as in any read-only buffer.

#### Scenario: Rope commands
- **WHEN** a terminal buffer is current and `forward-char`, `next-line`, `end-of-buffer` and `mark-whole-buffer` run in it with `M-x`
- **THEN** none panics and point stays at 0

#### Scenario: Edits
- **WHEN** `newline` runs with `M-x` in a terminal buffer
- **THEN** the echo area says `Buffer is read-only` and its grid is unchanged
