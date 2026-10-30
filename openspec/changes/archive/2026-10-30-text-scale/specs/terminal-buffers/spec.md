# Spec Delta

## MODIFIED Requirements

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live, it isn't in copy mode and no key sequence is pending, every key except `C-x`, `C-c`, `M-x`, keys with super held and, on Linux, `C-S-v` and the zoom keys `C-=`, `C-+`, `C--` and `C-0` SHALL be written to the shell, encoded as xterm does, instead of running a command. Those and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

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

#### Scenario: Zoom keys in a terminal
- **WHEN** the platform's zoom key is pressed in a terminal in char mode
- **THEN** nothing is written to the shell and the font size changes
