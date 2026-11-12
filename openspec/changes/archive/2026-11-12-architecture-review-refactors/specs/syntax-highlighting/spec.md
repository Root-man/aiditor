# Spec Delta

## MODIFIED Requirements

### Requirement: Highlighting follows the text
Highlighting SHALL reflect the buffer's current text after any edit, undo, redo or revert from disk. In a buffer of at most 64 KiB of text it SHALL do so whenever the buffer is drawn. In a larger buffer the colours MAY trail the text until the buffer's new parse arrives, and SHALL reflect the current text once a parse of that text has arrived.

#### Scenario: Typing makes a comment
- **WHEN** the user types `// ` at the start of a line of code in a JS buffer
- **THEN** the whole line is drawn in the `comment` colour

#### Scenario: Undo restores colours
- **WHEN** the user then undoes the typing
- **THEN** the line is drawn with the colours it had before

#### Scenario: Revert
- **WHEN** the file changes on disk and the unmodified buffer reverts to it
- **THEN** the buffer is drawn with the new text's colours

#### Scenario: A large buffer catches up
- **WHEN** the user types `// ` at the start of a line of code in a JS buffer larger than 64 KiB, and the parse that edit asked for arrives
- **THEN** the whole line is drawn in the `comment` colour

## ADDED Requirements

### Requirement: A large buffer is parsed outside the core
The core SHALL NOT parse a buffer larger than 64 KiB of text while it handles a message or builds a frame. It SHALL ask for the parse as an effect carrying the text and the last tree, and SHALL have at most one parse outstanding per buffer.

#### Scenario: An edit asks for a parse
- **WHEN** a char is typed in a Rust buffer larger than 64 KiB
- **THEN** handling the key returns one parse effect for that buffer, and the buffer's text has the char

#### Scenario: Edits during a parse ask for no second one
- **WHEN** two more chars are typed before the parse arrives
- **THEN** no further parse effect is returned for that buffer

#### Scenario: A small buffer asks for none
- **WHEN** a char is typed in a Rust buffer of 1 KiB
- **THEN** no parse effect is returned, and the next frame draws the buffer's current colours

#### Scenario: Opening a large file
- **WHEN** a Rust file larger than 64 KiB is visited
- **THEN** its text is shown at once without syntax colours, one parse effect is returned, and the colours are drawn once the parse arrives

### Requirement: Colours follow their text while a parse is outstanding
While a large buffer's parse is outstanding, the display SHALL draw the last parse's colours moved with the text the edits moved, so text the edits did not touch keeps its colours and its place.

#### Scenario: A line inserted above
- **WHEN** a new line is inserted above a function in a large Rust buffer and the parse has not arrived
- **THEN** the function's keywords are drawn in the `keyword` colour on their new lines

### Requirement: A late parse is applied to the text it finds
When a parse arrives for a buffer edited since the parse was asked for, the core SHALL keep the result, move it with those later edits, and ask for one more parse of the current text. A parse for a killed buffer SHALL be dropped.

#### Scenario: Typing through a parse
- **WHEN** a parse arrives after two more chars were typed
- **THEN** the buffer is drawn with that parse's colours moved by the two chars, and one new parse effect is returned

#### Scenario: Colours settle after the last edit
- **WHEN** the new parse arrives with no edit made since it was asked for
- **THEN** the buffer's colours are those of a fresh parse of its text, and no parse effect is returned

#### Scenario: The buffer was killed
- **WHEN** a parse arrives for a buffer that was killed
- **THEN** nothing changes and no effect is returned
