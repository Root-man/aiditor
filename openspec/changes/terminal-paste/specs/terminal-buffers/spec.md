## ADDED Requirements

### Requirement: s-v and C-S-v paste into the shell
`s-v` and `C-S-v` in a live terminal buffer SHALL write the kill at the yank pointer, the newest unless `M-y` moved it, to its shell, after taking in clipboard text from another app as `yank` does, instead of inserting it. With an empty kill ring and clipboard it SHALL echo `Kill ring is empty` and write nothing.

#### Scenario: Paste the clipboard
- **WHEN** the clipboard holds `ls` from another app and `s-v` is pressed in a terminal
- **THEN** the shell receives `ls`

#### Scenario: C-S-v where Super belongs to the desktop
- **WHEN** `C-S-v` is pressed in a terminal
- **THEN** it pastes as `s-v` does, and `C-v` still reaches the shell as `\x16`

### Requirement: Paste is bracketed when the program asks
While the program has bracketed paste on (mode 2004), pasted text SHALL be sent between `\x1b[200~` and `\x1b[201~` with every ESC in it removed; otherwise it SHALL be sent as is.

#### Scenario: Bracketed
- **WHEN** the program sent `\x1b[?2004h` and `a\nb\x1b[201~c` is pasted
- **THEN** the shell receives `\x1b[200~a\nb[201~c\x1b[201~`

#### Scenario: Not bracketed
- **WHEN** the program has not turned on bracketed paste and `a\nb` is pasted
- **THEN** the shell receives `a\nb`

## MODIFIED Requirements

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live and no key sequence is pending, every key except `C-x`, `C-c`, `M-x`, `C-S-v` and keys with super held SHALL be written to the shell, encoded as xterm does, instead of running a command. Those and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

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
