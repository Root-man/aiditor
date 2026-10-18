# Spec Delta

## ADDED Requirements

### Requirement: M-x runs a command by name
`M-x` SHALL read a command name at the prompt `M-x `, completing over every registered command in name order, and run the chosen command with the prefix argument `M-x` was given, as the command of that key sequence. A name that is not registered SHALL fail with a message naming it.

#### Scenario: Running a command by name
- **WHEN** in a buffer holding `abc` with point at 0, `C-u 2 M-x forward-char RET` is typed
- **THEN** point is at 2

#### Scenario: Narrowing to a command
- **WHEN** `M-x` is typed, then `split-below`
- **THEN** the selected candidate is `split-window-below`

#### Scenario: An unknown name
- **WHEN** `M-x zzz-nothing RET` is typed
- **THEN** the echo area shows an error naming `zzz-nothing`

### Requirement: M-x shows each command's key
Each `M-x` candidate SHALL be annotated with the shortest key sequence that runs it under keymap layering in the buffer `M-x` was called from, the first in `kbd` text order among equally short ones, and no annotation when no key runs it.

#### Scenario: Bindings in the list
- **WHEN** `M-x` is typed, then `split-window-b`
- **THEN** the candidate `split-window-below` is annotated `C-x 2`

#### Scenario: A command without a key
- **WHEN** `M-x` is typed, then `exit-minibuffer`
- **THEN** `exit-minibuffer` has no annotation, since no key runs it outside the minibuffer
