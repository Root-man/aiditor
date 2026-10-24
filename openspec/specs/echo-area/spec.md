# echo-area Specification

## Purpose
The echo area is the frame's bottom line, where the editor shows messages and errors, such as a failed command, an undefined key or `Quit`, until the user's next action.

## Requirements

### Requirement: Messages stay until the next key
A message SHALL stay in the echo area until the next key, which SHALL clear it before running whatever that key does. Timers SHALL NOT clear it.

#### Scenario: The next key clears an error
- **WHEN** a command fails with `boom` and then `a` is typed
- **THEN** the echo area showed `boom` after the failure and is blank after `a`

#### Scenario: A key that fails replaces the message
- **WHEN** `C-g` shows `Quit` and then `M-z`, which nothing binds, is pressed
- **THEN** the echo area shows `M-z is undefined`
