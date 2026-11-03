## ADDED Requirements

### Requirement: Replacement strings can break display lines
A replacement string containing a newline SHALL draw the text after it on the next display row without inserting that newline into the buffer. A replacement spanning hidden buffer lines SHALL leave subsequent text visible after the replacement.

#### Scenario: Paste header and preview
- **GIVEN** raw multiline text with a replacement header and display newline on its first character
- **WHEN** a frame is built
- **THEN** the header occupies one display row and the original first line starts on the following row, with the buffer text unchanged

## MODIFIED Requirements

### Requirement: A range can be drawn as other text
A buffer range MAY carry a display property listing pieces of text, each with its own face layers. The display SHALL draw the pieces in place of the range, one span per piece, counting their columns as it counts a fold's placeholder, with the cursor on the first piece when point is in the range, covering the pieces on the replacement's first display row when point is at its start. The buffer's text SHALL stay as it is.

#### Scenario: Drawn in its pieces' faces
- **WHEN** `a` in `xay` carries pieces ` `, `▤` in `status-failed` and ` n ` in `attachment`
- **THEN** the line's spans are `x`, ` `, `▤`, ` n ` and `y`, each piece in its face

#### Scenario: Right alignment counts the pieces
- **WHEN** a right-aligned time follows such a range on its line
- **THEN** the time still ends in the window's last column
