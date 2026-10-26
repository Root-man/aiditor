## ADDED Requirements

### Requirement: Properties in a range can be removed
A buffer SHALL remove every text property whose range lies within a given char range, leaving those that extend past it.

#### Scenario: Removing inside a range
- **WHEN** faces cover 1..3, 4..6 and 5..9 of `abcdefghij` and the properties within 0..6 are removed
- **THEN** only the face on 5..9 is left
