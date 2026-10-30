# Spec Delta

## MODIFIED Requirements

### Requirement: The prefix argument counts motions
`forward-char`, `backward-char`, `next-line`, `previous-line`, `forward-word` and `backward-word` SHALL move by the prefix argument's value, 1 without one, and a negative value SHALL move the other way.

#### Scenario: C-u 3 C-f
- **WHEN** point is 0 in `abcdef` and `C-u 3 C-f` is pressed
- **THEN** point is 3

#### Scenario: C-u alone counts four
- **WHEN** point is 0 in `abcdef` and `C-u C-f` is pressed
- **THEN** point is 4

#### Scenario: Negative count reverses
- **WHEN** point is 5 in `abcdef` and `M-- 2 C-f` is pressed
- **THEN** point is 3
