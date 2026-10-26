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

### Requirement: Markers are positions that follow edits
A buffer SHALL hold markers, each a char offset with an insertion type. Adding a marker at an offset past the end of the text, or moving one there, SHALL fail. A marker can be read until it is removed.

#### Scenario: Add and read a marker
- **WHEN** a marker is added at offset 2 of `hello`
- **THEN** reading it gives 2

#### Scenario: Marker past the end
- **WHEN** a marker is added at offset 6 of `hello`
- **THEN** adding it fails

#### Scenario: Removed marker is gone
- **WHEN** a marker is removed
- **THEN** reading it finds nothing

### Requirement: Inserts move markers by insertion type
An insert of n chars at offset i SHALL move markers after i forward by n and leave markers before i in place. A marker exactly at i SHALL stay at i if its insertion type is stay, and move to i + n if it is advance.

#### Scenario: Insert before, at and after markers
- **WHEN** `XY` is inserted at offset 2 of `abcd` with markers at 1, 2 and 3, for each insertion type
- **THEN** the markers at 1 stay at 1, the markers at 3 move to 5, the stay marker at 2 stays at 2 and the advance marker at 2 moves to 4

### Requirement: Deletes move markers to the deleted range's start
A delete of the range a..b SHALL leave markers at or before a in place, move markers at or after b back by b - a, and move markers inside the range to a, whatever their insertion type.

#### Scenario: Delete before, at and after markers
- **WHEN** the range 1..3 is deleted from `abcde` with markers at 0, 1, 2, 3 and 4, for each insertion type
- **THEN** the markers are at 0, 1, 1, 1 and 2

### Requirement: Refused edits leave markers in place
An edit the buffer refuses SHALL NOT move any marker.

#### Scenario: Insert past the end
- **WHEN** text is inserted at offset 6 of `hello` with a marker at 3
- **THEN** the marker is still at 3

### Requirement: Text properties follow edits
A buffer SHALL hold text properties, each a face, `invisible` with a placeholder, or a line decoration on a char range. Adding one on a range past the end SHALL fail. Text inserted inside a range SHALL extend it; text inserted at its start or end SHALL not. A deletion SHALL shrink the range, and a range deleted whole SHALL have no effect.

#### Scenario: Insert inside and at the edges
- **WHEN** a face covers 2..4 of `abcdef`, then `X` is inserted at 3, at 2 and at 6
- **THEN** the face covers 3..6

#### Scenario: Range deleted whole
- **WHEN** a face covers 2..4 and 1..5 is deleted
- **THEN** no text has the face

#### Scenario: Range past the end
- **WHEN** a face is added on 3..9 of `hello`
- **THEN** adding it fails

### Requirement: Properties in a range can be removed
A buffer SHALL remove every text property whose range lies within a given char range, leaving those that extend past it.

#### Scenario: Removing inside a range
- **WHEN** faces cover 1..3, 4..6 and 5..9 of `abcdefghij` and the properties within 0..6 are removed
- **THEN** only the face on 5..9 is left
