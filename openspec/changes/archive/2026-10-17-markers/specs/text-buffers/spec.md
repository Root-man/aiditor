## ADDED Requirements

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
