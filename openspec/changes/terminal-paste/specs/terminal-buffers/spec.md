## ADDED Requirements

### Requirement: s-v pastes into the shell
`s-v` in a live terminal buffer SHALL write the kill at the yank pointer, the newest unless `M-y` moved it, to its shell, after taking in clipboard text from another app as `yank` does, instead of inserting it. With an empty kill ring and clipboard it SHALL echo `Kill ring is empty` and write nothing.

#### Scenario: Paste the clipboard
- **WHEN** the clipboard holds `ls` from another app and `s-v` is pressed in a terminal
- **THEN** the shell receives `ls`

### Requirement: Paste is bracketed when the program asks
While the program has bracketed paste on (mode 2004), pasted text SHALL be sent between `\x1b[200~` and `\x1b[201~` with every ESC in it removed; otherwise it SHALL be sent as is.

#### Scenario: Bracketed
- **WHEN** the program sent `\x1b[?2004h` and `a\nb\x1b[201~c` is pasted
- **THEN** the shell receives `\x1b[200~a\nb[201~c\x1b[201~`

#### Scenario: Not bracketed
- **WHEN** the program has not turned on bracketed paste and `a\nb` is pasted
- **THEN** the shell receives `a\nb`
