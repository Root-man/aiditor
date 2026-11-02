# Spec Delta

## ADDED Requirements

### Requirement: Two spaces can carry an icon

A range of exactly two chars in a buffer MAY carry an icon property naming one of the bundled icons: book, magnifier, pencil, terminal, globe or wrench. The buffer's text SHALL stay as it is, and the chars SHALL be drawn, moved over, searched, killed and copied as the plain spaces they are. Each row of the display SHALL say at which of its columns an icon starts, and which. An icon SHALL be placed only when both its cells are on the row: one cut by a row break or by the window's edge, or whose range no longer holds two chars, SHALL leave plain spaces.

#### Scenario: The display places the icon

- **WHEN** `a  b` has the book icon on its two spaces
- **THEN** the row's text is `a  b`, and it places the book at column 1

#### Scenario: Text around it moves the column on wrapped rows

- **WHEN** the icon falls on the second row of a wrapped line, past the row's start
- **THEN** that row places it at its column in that row, and the first row places none

#### Scenario: A row break cuts the icon

- **WHEN** a row breaks between the icon's two spaces
- **THEN** neither row places an icon, and both spaces are drawn

#### Scenario: Horizontal scrolling cuts the icon

- **WHEN** a truncated line is scrolled so that only the icon's second cell shows
- **THEN** the row places no icon

#### Scenario: Faces split the cells

- **WHEN** the region ends between the icon's two spaces
- **THEN** the row still places one icon at its first cell

#### Scenario: Point moves through its cells

- **WHEN** point is before the icon and `forward-char` runs twice
- **THEN** point is past the icon's second space

#### Scenario: Search finds the spaces

- **WHEN** a search for two spaces runs over the icon's line
- **THEN** it matches the icon's range

#### Scenario: Kill takes the spaces

- **WHEN** the icon's range is killed and yanked elsewhere
- **THEN** the kill ring holds two spaces, and the yanked text carries no icon
