# Spec Delta

## Purpose

The echo area is the frame's bottom line, where the editor shows messages and errors, such as a failed command, an undefined key or `Quit`, until the user's next action.

## ADDED Requirements

### Requirement: Messages stay until the next key or click
A message SHALL stay in the echo area until the next key or mouse click, which SHALL clear it before running whatever that key or click does. Scrolling and timers SHALL NOT clear it.

#### Scenario: The next key clears an error
- **WHEN** a command fails with `boom` and then `a` is typed
- **THEN** the echo area showed `boom` after the failure and is blank after `a`

#### Scenario: A click clears a message
- **WHEN** `C-g` shows `Quit` and then the window is clicked
- **THEN** the echo area is blank

#### Scenario: A key that fails replaces the message
- **WHEN** `C-g` shows `Quit` and then `M-z`, which nothing binds, is pressed
- **THEN** the echo area shows `M-z is undefined`
