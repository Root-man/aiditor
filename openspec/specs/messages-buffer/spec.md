# messages-buffer Specification

## Purpose
The `*Messages*` buffer keeps every echo-area message and the app's warnings and errors, so what flashed by in the echo area or went to stderr can be read back in the editor.

## Requirements

### Requirement: Echo-area messages are logged to Messages
Every message shown in the echo area SHALL be appended to the end of `*Messages*` as its own line, in the order shown, including a message repeated after the echo area was cleared.

#### Scenario: Quit is logged each time
- **WHEN** `C-g` is typed twice
- **THEN** `*Messages*` holds `Quit\nQuit\n`

#### Scenario: An undefined key is logged
- **WHEN** `M-z`, which nothing binds, is pressed
- **THEN** the last line of `*Messages*` is `M-z is undefined`

### Requirement: Warnings and errors the app logs are appended to Messages
Each warning or error the app logs SHALL be appended to the end of `*Messages*` as a line, with the text the stderr log gets, without terminal colour codes. Lines below warning level SHALL NOT be appended; stderr's log is unchanged.

#### Scenario: A warning arrives
- **WHEN** the app logs `careful` at warn level
- **THEN** a line of `*Messages*` contains `WARN` and `careful`

#### Scenario: Info stays out
- **WHEN** the app logs `starting` at info level
- **THEN** no line of `*Messages*` contains `starting`

### Requirement: Messages is read-only and made on demand
`*Messages*` SHALL be made when its first line arrives, and made again on the next line after it is killed. Its edits SHALL fail with `Buffer is read-only`, and appending SHALL NOT add to its undo history.

#### Scenario: Typing in Messages fails
- **WHEN** a message was shown, `*Messages*` is switched to and `a` is typed
- **THEN** the echo area shows `Buffer is read-only` and the buffer is unchanged apart from that line

#### Scenario: Killed and made again
- **WHEN** `*Messages*` is killed and then `C-g` is typed
- **THEN** a new `*Messages*` holds `Quit\n`

#### Scenario: No buffer before the first message
- **WHEN** the editor starts and nothing has been shown or logged
- **THEN** there is no `*Messages*` buffer
