## ADDED Requirements

### Requirement: Key sequences name the super modifier
A key sequence SHALL be able to name the super modifier, the Cmd key on macOS, as `s-` in `kbd` notation, written after `C-`, `M-` and `S-`, and a key with super held SHALL match only a binding that names it.

#### Scenario: Super parses and prints
- **WHEN** the sequence `C-s-a s-z` is parsed and written back
- **THEN** it reads `C-s-a s-z`, and its first key has Ctrl and super held

#### Scenario: Super keys are distinct
- **WHEN** `s-z` is pressed
- **THEN** it runs the command bound to `s-z`, not the one bound to `z`

### Requirement: The bundled keymap binds Cmd keys
The bundled global keymap SHALL bind `s-z` to `undo` and `s-s` to `save-buffer`.

#### Scenario: Cmd-Z undoes
- **WHEN** a char is typed and `s-z` is pressed
- **THEN** the char is undone
