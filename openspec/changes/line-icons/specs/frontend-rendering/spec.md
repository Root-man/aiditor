# Spec Delta

## ADDED Requirements

### Requirement: Icons are painted over their two cells

An icon on a window's text row SHALL be painted as a monochrome SVG from the set bundled in the binary, in the foreground colour of the run its first cell is in, centred on its two cells and on its row, as wide and tall as the font size or the two cells' width, whichever is less. It SHALL NOT move any text. The block cursor SHALL paint over it, as it does over text.

#### Scenario: It takes the colour of its run

- **WHEN** an icon's spaces are in a face with a foreground, in any theme
- **THEN** the icon is painted in that face's foreground

#### Scenario: No face uses the default foreground

- **WHEN** an icon's spaces have no face, in a dark theme and in a light one
- **THEN** the icon is painted in that theme's foreground

#### Scenario: It spans its two cells

- **WHEN** an icon sits between text on a row
- **THEN** its bounds are centred on the x of its two spaces, and every char is at the x it has without the icon

#### Scenario: Marked text shifts it

- **WHEN** IME marked text is spliced in before the icon
- **THEN** the icon is painted at its spaces' shaped position, past the marked text

#### Scenario: Every icon is bundled

- **WHEN** the frontend loads each icon's asset
- **THEN** it gets an SVG
