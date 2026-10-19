# Spec Delta

## MODIFIED Requirements

### Requirement: The cursor is drawn over the line without moving its text
The selected window SHALL draw the cursor as a block in the theme's cursor color at the x offset of point's column in its laid-out line, over the text, as wide as the char at point or one cell at the end of the line, with that char drawn over the block in the background color. Point's line SHALL be laid out exactly as any other line, so moving point never shifts any glyph. Other windows SHALL draw no cursor.

#### Scenario: Moving point keeps glyphs still
- **WHEN** point moves from the start to the middle of a line
- **THEN** the line's laid-out text is the same and the cursor moves to that column's x offset

#### Scenario: A block over the char
- **WHEN** point is before a char
- **THEN** the cursor is as wide as that char

### Requirement: The key hint is laid out in columns
While the key hint shows, it SHALL be drawn above the echo area as the pending keys then each entry, `key : command`, in columns of equal width, as many per row as fit the frame's width and as many rows as needed. The windows SHALL shrink to make room.

#### Scenario: C-x hint
- **WHEN** `C-x` waits past the hint delay in a wide frame
- **THEN** its entries, such as `u : undo`, are drawn in several columns above the echo area

## ADDED Requirements

### Requirement: The frame keeps a gap at its edges
The frame's content SHALL be inset 8 px from the left and right edges and 2 px from the bottom, so rounded window corners never clip text.

#### Scenario: Minibuffer at the corner
- **WHEN** `M-x` is typed
- **THEN** the prompt's first letter starts 8 px from the frame's left edge
