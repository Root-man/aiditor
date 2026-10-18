# Spec Delta

## MODIFIED Requirements

### Requirement: M-d and M-DEL delete words
`M-d` SHALL run `kill-word`, which kills from point to where `forward-word` with the same prefix argument would move. `M-DEL` SHALL run `backward-kill-word`, which does the same as `backward-word` would move. The killed text SHALL be saved on the kill ring like any other kill.

#### Scenario: Kill a word forward
- **WHEN** in `foo bar baz` with point at 0, `M-d` is pressed
- **THEN** the text is ` bar baz` and point is 0

#### Scenario: Kill words forward by a count
- **WHEN** in `foo bar baz` with point at 0, `C-u 2 M-d` is pressed
- **THEN** the text is ` baz`

#### Scenario: Kill a word backward
- **WHEN** in `foo bar` with point at 7, `M-DEL` is pressed
- **THEN** the text is `foo ` and point is 4

#### Scenario: The killed word yanks back
- **WHEN** in `foo bar` with point at 7, `M-DEL C-y` is pressed
- **THEN** the text is `foo bar`
