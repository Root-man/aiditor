# Spec Delta

## ADDED Requirements

### Requirement: Text properties follow edits
A buffer SHALL hold text properties, each a face, `invisible` with a placeholder, or a line decoration on a char range. Adding one on a range past the end SHALL fail. Text inserted inside a range SHALL extend it; text inserted at its start or end SHALL not. A deletion SHALL shrink the range, and a range deleted whole SHALL have no effect.

#### Scenario: Insert inside and at the edges
- **WHEN** a face covers 2..4 of `abcdef`, then `X` is inserted at 3, at 2 and at 6
- **THEN** the face covers 3..6

#### Scenario: Range deleted whole
- **WHEN** a face covers 2..4 and 1..5 is deleted
- **THEN** no text has the face

#### Scenario: Range past the end
- **WHEN** a face is added on 3..9 of `hello`
- **THEN** adding it fails
