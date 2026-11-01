# Spec Delta

## ADDED Requirements

### Requirement: Listed hits are cut to the visible width
A listed hit's row SHALL hold no more of its line's text than the list's columns can show. The hit itself SHALL keep the whole line, so previewing it and `RET` still reach the match wherever it is on the line.

#### Scenario: A very long line
- **GIVEN** a file whose only line is 1 MB long and matches
- **WHEN** the hit is listed in a list 80 columns wide
- **THEN** its row holds at most 80 chars of the line's text

#### Scenario: A short line
- **WHEN** a hit's line fits the list's columns
- **THEN** its row holds the whole line

#### Scenario: A match past the edge
- **GIVEN** a hit whose match starts past the list's columns
- **WHEN** `RET` is pressed on it
- **THEN** point is at the match
