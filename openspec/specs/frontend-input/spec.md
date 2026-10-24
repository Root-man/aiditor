# frontend-input Specification

## Purpose
How key presses, typed text and input-method text from the platform reach the core, so every key binding works and dead keys and IMEs type text.

## Requirements

### Requirement: Command keys reach the core as keys
A key pressed with Ctrl or Cmd, with Option or Alt when it is Meta, `RET`, `TAB`, `DEL`, `ESC`, an arrow, or a key that types no text SHALL be sent to the core as a key with its modifiers, named by the key's own character, and SHALL not reach the platform's text input. Every key sequence in the bundled keymap SHALL reach the core this way on macOS and Linux.

#### Scenario: Every bundled binding reaches the core
- **WHEN** each key sequence in the bundled keymap is pressed in the frontend
- **THEN** the core receives exactly those keys

#### Scenario: Option as Meta
- **WHEN** Option-e is pressed with `option_as_meta` true, or Alt-e on Linux
- **THEN** the core receives `M-e`

### Requirement: Typing reaches the core as text
Any other key SHALL go to the platform's text input, and the text it produces SHALL reach the core as inserted text. Each inserted char SHALL then act as a typed plain char: it extends a pending prefix or prefix argument, runs what a keymap binds it to, or inserts itself.

#### Scenario: Typed text inserts
- **WHEN** `a` and shifted `b` are typed
- **THEN** the buffer holds `aB`

#### Scenario: Typed chars finish a prefix
- **WHEN** `C-x` is pressed and `u` typed
- **THEN** `undo` runs and nothing is inserted

#### Scenario: Typed digits extend the prefix argument
- **WHEN** `C-u` is pressed and `3` and `a` typed
- **THEN** the buffer holds `aaa`

#### Scenario: Option for characters
- **WHEN** `option_as_meta` is false on macOS and Option-e then `e` is typed
- **THEN** the dead key types `é`

### Requirement: Input methods can replace text already typed
Inserted text MAY name a range of the buffer in UTF-16 code units to replace. The core SHALL delete that range, insert the text at its start as typed chars, and keep it undoable. A range outside the buffer or reversed SHALL change nothing and report an error.

#### Scenario: Press and hold
- **WHEN** `e` is typed and the input method replaces UTF-16 range 0..1 with `é`
- **THEN** the buffer holds `é` and one undo removes it

#### Scenario: Range past a non-BMP char
- **WHEN** the buffer holds `😀e`, point is at its end and the range 2..3 is replaced with `é`
- **THEN** the buffer holds `😀é`

#### Scenario: Bad range
- **WHEN** a replace range ends past the buffer's end
- **THEN** the buffer is unchanged and an error is shown

### Requirement: Marked text stays out of the buffer
Text an input method marks while composing SHALL be drawn at point inside point's line, underlined, with the cursor after it, and SHALL not be in the buffer, its undo history or keyboard macros. Committing it SHALL insert the committed text as typed text; cancelling it SHALL insert nothing.

#### Scenario: Dead key composes
- **WHEN** the input method marks `´` and then commits `é`
- **THEN** the buffer is unchanged while `´` is marked and holds `é` afterwards

### Requirement: Navigation and function keys reach the core
Home, End, PageUp, PageDown, Delete and F1 to F12 SHALL be sent to the core as keys with their modifiers.

#### Scenario: F1 and Home
- **WHEN** `f1` and `shift-home` are pressed
- **THEN** the core receives `<f1>` and `S-<home>`
