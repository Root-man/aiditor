## MODIFIED Requirements

### Requirement: Wide chars take two cells
A wide char SHALL take two columns, its run counting two cells, so the chars after it sit at the same columns as in the rows below. An emoji such as `🦀` is a wide char.

#### Scenario: Japanese text
- **WHEN** `日本x\r\nabcde` is fed
- **THEN** `x` is at column 4 on the first row, the same column as `e` on the second

#### Scenario: Emoji
- **WHEN** `🦀x\r\nabc` is fed
- **THEN** `🦀` is a run of two cells at column 0 and `x` is at column 2, the same column as `c` on the second row
