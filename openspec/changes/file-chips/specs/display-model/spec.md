## ADDED Requirements

### Requirement: A range can be drawn as other text
A buffer range MAY carry a display property listing pieces of text, each with its own face layers. The display SHALL draw the pieces in place of the range, one span per piece, counting their columns as it counts a fold's placeholder, with the cursor on the first piece when point is in the range. The buffer's text SHALL stay as it is.

#### Scenario: Drawn in its pieces' faces
- **WHEN** `a` in `xay` carries pieces ` `, `▤` in `status-failed` and ` n ` in `attachment`
- **THEN** the line's spans are `x`, ` `, `▤`, ` n ` and `y`, each piece in its face

#### Scenario: Right alignment counts the pieces
- **WHEN** a right-aligned time follows such a range on its line
- **THEN** the time still ends in the window's last column

### Requirement: Faces can strike through
A face SHALL optionally set `strike`, and a span SHALL carry it; the frontend SHALL draw a struck span with a line through it in its foreground colour.

#### Scenario: A struck face
- **WHEN** a face with `strike = true` covers a run
- **THEN** that run's span is struck
