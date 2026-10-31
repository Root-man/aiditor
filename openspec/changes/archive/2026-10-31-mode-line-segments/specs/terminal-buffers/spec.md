# Spec Delta

## MODIFIED Requirements

### Requirement: The mode line shows the terminal's mode
A terminal window's mode line SHALL show `[char]` as a right group segment in char mode, without point's position, and `[copy]` followed by a segment with point's line and column in copy mode.

#### Scenario: Char mode
- **WHEN** a terminal buffer `t` is shown in char mode
- **THEN** its left group is `%% t` and its right group `[char]`

#### Scenario: Copy mode
- **WHEN** copy mode is entered with point on line 5001, column 1
- **THEN** the left group is `%% t` and the right group's segments are `[copy]` and `5001:1`
