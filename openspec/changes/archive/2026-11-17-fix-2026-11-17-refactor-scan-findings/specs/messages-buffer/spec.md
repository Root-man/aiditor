## ADDED Requirements

### Requirement: Messages keeps its last 1,000 lines
When a line appended to `*Messages*` brings it past 1,000 lines, its oldest lines SHALL be removed until 1,000 remain, without adding to its undo history. Windows showing it SHALL keep point on the same text when that text remains.

#### Scenario: The oldest line goes
- **WHEN** `*Messages*` holds 1,000 lines and another message is shown
- **THEN** it holds 1,000 lines, the first is what was the second, and the last is the new message
