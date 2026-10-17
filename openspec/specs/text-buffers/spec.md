# text-buffers Specification

## Purpose
Buffers are the editor's units of content: each has an id, a name, editable text addressed by char offset, line/column positions, a modified flag and an optional file binding.

## Requirements

### Requirement: Buffers are created, found and killed by id
The core SHALL keep a registry of buffers. Creating a buffer returns a new id that is never reused in the same run. A buffer can be looked up by its id until it is killed; after that, the lookup finds nothing.

#### Scenario: Create and look up
- **WHEN** a buffer named `notes` is created
- **THEN** looking up the returned id gives a buffer named `notes` with empty text

#### Scenario: Ids are not reused
- **WHEN** a buffer is created, killed, and another buffer is created
- **THEN** the second buffer's id differs from the first

#### Scenario: Killed buffer is gone
- **WHEN** a buffer is killed
- **THEN** looking up its id finds nothing, and killing it again reports that it does not exist

### Requirement: Text is inserted and deleted by char offset
A buffer SHALL insert text at a char offset and delete a char range, where offsets count Unicode scalar values, not bytes.

#### Scenario: Insert into non-ASCII text
- **WHEN** `ö` is inserted at offset 2 of `wrld`
- **THEN** the text is `wröld`

#### Scenario: Delete a range
- **WHEN** the range 1..3 is deleted from `héllo`
- **THEN** the text is `hlo`

### Requirement: Invalid edits are refused without changing the buffer
An insert past the end of the text, or a delete whose range is reversed or ends past the end, SHALL return an error and leave the text and the modified flag unchanged. It SHALL NOT panic.

#### Scenario: Insert past the end
- **WHEN** text is inserted at offset 6 of `hello`
- **THEN** the edit fails and the text is still `hello`

#### Scenario: Delete past the end
- **WHEN** the range 3..9 is deleted from `hello`
- **THEN** the edit fails and the text is still `hello`

### Requirement: Offsets convert to and from line and column
The buffer SHALL convert a char offset to a zero-based (line, column) pair and back, where the column counts chars from the start of the line. Only LF breaks lines; CR and Unicode line separators are ordinary chars. A column past the end of its line SHALL clamp to the end of the line, before its LF; a line past the last line SHALL be an error.

#### Scenario: Offset to line and column in non-ASCII text
- **WHEN** offset 8 of `añb\nçd🙂e` is converted
- **THEN** the result is line 1, column 4

#### Scenario: Line and column to offset
- **WHEN** line 1, column 2 of `añb\nçd🙂e` is converted
- **THEN** the result is offset 6

#### Scenario: Column clamps to the end of the line
- **WHEN** line 0, column 99 of `ab\ncd` is converted
- **THEN** the result is offset 2

#### Scenario: Only LF breaks lines
- **WHEN** the text is `a\rb\u{2028}c\r\nd`
- **THEN** the buffer has 2 lines, and offset 7 is line 1, column 0

#### Scenario: Line past the end
- **WHEN** line 5 of a two-line buffer is converted
- **THEN** the conversion fails

### Requirement: The modified flag tracks unsaved edits
A new buffer, or one created with text from a file, SHALL start unmodified. Any edit that changes the text SHALL set the flag; an edit that changes nothing, such as inserting an empty string, SHALL NOT. Marking the buffer saved SHALL clear it.

#### Scenario: Edit sets the flag
- **WHEN** a char is inserted into an unmodified buffer
- **THEN** the buffer is modified

#### Scenario: Empty edit leaves it clear
- **WHEN** an empty string is inserted into an unmodified buffer
- **THEN** the buffer is still unmodified

#### Scenario: Marking saved clears it
- **WHEN** a modified buffer is marked saved
- **THEN** it is unmodified and its text is unchanged

### Requirement: A buffer can be bound to a file
A buffer SHALL have an optional file path. A buffer created for a file SHALL hold the given text, the path and the file's name as its buffer name, and start unmodified.

#### Scenario: Buffer for a file
- **WHEN** a buffer is created for `/tmp/proj/src/main.rs` with text `fn main() {}`
- **THEN** its path is `/tmp/proj/src/main.rs`, its name is `main.rs`, its text is `fn main() {}` and it is unmodified

#### Scenario: Scratch buffer has no file
- **WHEN** a buffer is created by name only
- **THEN** it has no file path

### Requirement: The core starts with a scratch buffer that typing edits
The core SHALL start with one buffer named `*scratch*`, with no file, and typed text SHALL be inserted into it through the buffer edit API.

#### Scenario: Typing into scratch
- **WHEN** the keys `hello RET wörld` are fed to a new core
- **THEN** the scratch buffer's text is `hello\nwörld` and it is modified
