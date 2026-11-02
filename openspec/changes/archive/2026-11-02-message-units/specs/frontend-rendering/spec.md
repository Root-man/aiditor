# Spec Delta

## MODIFIED Requirements

### Requirement: Lines are drawn whole with their spans' faces
Each display line SHALL be laid out as one piece of text in the settings' font family and size, each span in its face's foreground and background colors, the default foreground where a face sets none. A line background decoration SHALL fill the line's width, a rule decoration SHALL be drawn 2 px wide at the line's left edge, and a bar decoration SHALL be drawn 3 px wide at the line's left edge, in its face's foreground.

#### Scenario: Region shown
- **WHEN** the mark is active at 0 and point at 3 in `hello`
- **THEN** `hel` is drawn on the region color and `lo` on the background

#### Scenario: The message bar
- **WHEN** a line has a bar decoration in `message-at-point`
- **THEN** a 3 px bar in that face's foreground runs down the line's left edge, over any line background
