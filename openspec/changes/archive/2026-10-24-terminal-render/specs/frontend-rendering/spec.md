# Spec Delta

## ADDED Requirements

### Requirement: Terminal cells are drawn on the grid
A terminal window SHALL draw each run at its column times the cell width, each glyph advancing one cell, so wide chars and glyphs from fallback fonts never shift the rest of the row. A run's background SHALL fill its cells. Bold and italic runs SHALL use the font's bold weight and italic style, underlined runs an underline.

#### Scenario: A wide char keeps the row aligned
- **WHEN** a terminal shows `日本x` above `abcde`
- **THEN** `x` is drawn at the same x as `e`

### Requirement: The terminal cursor is drawn in its shape
A terminal window's cursor SHALL be drawn in the theme's cursor colour as a block over its cells, with the char under it in the background colour, as a beam at the cell's left edge, or as an underline at the cell's bottom, as the display says. In a window that isn't selected it SHALL be drawn as a hollow block outline whatever its shape. With no cursor in the display, none SHALL be drawn.

#### Scenario: Unselected window
- **WHEN** a terminal is shown in two windows
- **THEN** the selected one draws its cursor filled and the other draws a hollow outline
