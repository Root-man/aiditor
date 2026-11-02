# Spec Delta

## ADDED Requirements

### Requirement: Faces carry a box colour

A face SHALL optionally name a box colour, and a span SHALL carry the box colour of its face. A layer with a box colour SHALL win over those under it, and one without SHALL keep the colour under it. A box SHALL NOT change a span's text.

#### Scenario: A boxed property reaches its span

- **WHEN** a face with a box colour covers `bar` in `foo bar baz`
- **THEN** the line's spans are `foo `, `bar` and ` baz`, and only `bar` carries the colour

#### Scenario: The region keeps a box

- **WHEN** the region covers a boxed run
- **THEN** the run takes the region's background and keeps its box colour

### Requirement: A space can right-align the rest of its line

A space char with the right-align property SHALL be drawn as enough spaces, in that char's face, to end the rest of its display line in the window's last column. The rest of the line SHALL be counted as it is drawn, folds as their placeholders. When the line with one space does not fit in a row, the char SHALL be drawn as one space, and the line SHALL wrap or be cut as any other. Before the window's width is known, it SHALL be drawn as one space. The buffer's text SHALL NOT change, and a padded line SHALL NOT wrap.

#### Scenario: The text ends in the last column

- **WHEN** `ab 12s` has the property on its space in a truncating window 20 columns wide
- **THEN** the line is drawn 20 columns wide, `ab` at its start and `12s` at its end

#### Scenario: A wrapped window reaches the same column

- **WHEN** the same line is in a wrapped window 20 columns wide
- **THEN** it is one row, 20 columns wide

#### Scenario: A resize moves the text

- **WHEN** the window is resized to 30 columns
- **THEN** `12s` ends in column 30

#### Scenario: A line too long falls back to one space

- **WHEN** a line with the property needs more columns than a row holds
- **THEN** it is drawn with one space there and breaks into rows as without the property

#### Scenario: A fold counts as its placeholder

- **WHEN** the text after the space holds a fold
- **THEN** the line ends in the last column with the placeholder drawn

#### Scenario: The padding takes the space's face

- **WHEN** the space has a face with a background
- **THEN** the padding has that background

#### Scenario: The cursor follows the text

- **WHEN** point is on the `1` of the right-aligned `12s`
- **THEN** the cursor is on that `1` where it is drawn, and on the first padding column when point is on the space

#### Scenario: Text is unchanged

- **WHEN** the line is searched or killed
- **THEN** the search and the kill see one space
