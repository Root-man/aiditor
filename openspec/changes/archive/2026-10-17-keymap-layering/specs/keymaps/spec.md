# Spec Delta

## Purpose

Resolves key sequences to commands through layered keymaps with prefix keys, so modes can add or shadow bindings and the user hears about keys that do nothing.

## ADDED Requirements

### Requirement: Key sequences resolve through prefix keys
A keymap SHALL bind a key sequence either to a command or as a prefix. After a prefix key the core SHALL wait for the next key and look up the longer sequence; when a sequence is bound to a command, that command SHALL run and the pending sequence SHALL be cleared.

#### Scenario: Two-level prefix
- **WHEN** a test binds `C-x 4 f` to a command and presses `C-x 4 f`
- **THEN** the command runs once and the next key starts a new sequence

#### Scenario: Existing bindings still work
- **WHEN** `C-x C-s` is pressed in a buffer visiting a file
- **THEN** the buffer is saved, as before

### Requirement: Keymaps are layered
The core SHALL look up the pending sequence in the transient map, then each minor-mode map of the current buffer in order, then its major-mode map, then the global map. The first map that binds the sequence, as a command or a prefix, SHALL decide it.

#### Scenario: A minor mode shadows a global binding
- **WHEN** the global map binds `C-c a` to one command, a minor mode of the current buffer binds it to another, and `C-c a` is pressed
- **THEN** the minor mode's command runs

#### Scenario: Removing the mode restores the global binding
- **WHEN** that minor mode is removed from the buffer and `C-c a` is pressed again
- **THEN** the global map's command runs

#### Scenario: A mode prefix does not hide other global keys under it
- **WHEN** a minor mode binds `C-x a` and `C-x C-s` is pressed
- **THEN** the global `C-x C-s` runs

#### Scenario: The major mode sits below minor modes and above global
- **WHEN** the global map, the major mode and a minor mode all bind one key, and the minor mode is then removed
- **THEN** the minor mode's command runs first, then the major mode's

#### Scenario: The transient map wins once
- **WHEN** a transient map binds a key that the global map also binds, and the key is pressed twice
- **THEN** the transient command runs the first time and the global one the second time

### Requirement: Undefined keys are reported
A sequence that no layer binds SHALL be dropped and the message line SHALL show it in `kbd` form followed by `is undefined`. A single key with neither Ctrl nor Meta that no layer binds SHALL instead keep its editing behavior (typing, `RET`, `DEL`, arrows); unbound `TAB` and `ESC` have none and are reported.

#### Scenario: Undefined after a prefix
- **WHEN** `C-x C-q` is pressed and nothing binds it
- **THEN** the buffer is unchanged and the message line shows `C-x C-q is undefined`

#### Scenario: Undefined modified key
- **WHEN** `M-z` is pressed and nothing binds it
- **THEN** the message line shows `M-z is undefined`

#### Scenario: Unbound TAB
- **WHEN** `TAB` is pressed and nothing binds it
- **THEN** the message line shows `TAB is undefined`

#### Scenario: Plain keys still type
- **WHEN** `ab` is typed
- **THEN** the text is `ab` and no message is shown

### Requirement: C-g quits
`C-g` after a prefix key SHALL cancel the pending sequence without running a command, and the message line SHALL show `Quit`. `C-g` with no pending prefix SHALL run the `keyboard-quit` command, which shows `Quit`.

#### Scenario: Cancel a prefix
- **WHEN** `C-x C-g` is pressed and then `C-s`
- **THEN** nothing is saved, the message line showed `Quit`, and `C-s` is looked up on its own

#### Scenario: C-g on its own
- **WHEN** `C-g` is pressed with no prefix pending
- **THEN** the message line shows `Quit`
