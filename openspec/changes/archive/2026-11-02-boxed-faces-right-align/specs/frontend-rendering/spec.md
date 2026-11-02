# Spec Delta

## ADDED Requirements

### Requirement: Boxed runs are drawn around their cells

A run in a window's text rows whose face has a box colour SHALL be drawn with a 1px border in that colour, radius 5px, at the run's horizontal bounds and 1px inside its row at top and bottom, filled with the run's background. The box SHALL NOT move the run's text or any text after it. Adjacent runs with the same box colour SHALL share one outline, each keeping its own colours inside it. A run split across rows SHALL have a closed box on each row. The bundled `attachment` face SHALL have a box colour.

#### Scenario: The box keeps the run's width

- **WHEN** a boxed run sits between plain text on a row
- **THEN** every char is at the x it has without the box, and the box spans exactly the run's chars

#### Scenario: Two colours in one chip

- **WHEN** a boxed icon in one face is followed by a boxed name in another, with the same box colour
- **THEN** one outline encloses both, open between them

#### Scenario: Different box colours stay apart

- **WHEN** adjacent runs have different box colours
- **THEN** each has its own closed box

#### Scenario: Marked text splits a box

- **WHEN** IME marked text is spliced into a boxed run
- **THEN** the run's pieces are boxed around the marked text at their shaped positions

#### Scenario: A theme sets the colour

- **WHEN** a theme gives a face `box = 0x504945`
- **THEN** that face's runs are boxed in that colour
