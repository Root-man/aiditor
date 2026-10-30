# Spec Delta

## MODIFIED Requirements

### Requirement: Zoom keys change the font size
`text-scale-increase` and `text-scale-decrease` SHALL change the frame's font size by 1px, staying within 6 to 72. Line heights, mode lines, gutter digits, the minibuffer and terminal grids SHALL follow the new size, and the workspace modeline's size SHALL take the same offset, within the same range. The text area of each window SHALL be laid out again at the new size.

#### Scenario: Increase and decrease
- **GIVEN** the font size is 14
- **WHEN** `text-scale-increase` runs twice and `text-scale-decrease` runs once
- **THEN** the font size is 15 and the line height follows it

#### Scenario: The range holds
- **GIVEN** the font size is 72
- **WHEN** `text-scale-increase` runs
- **THEN** the font size stays 72, and likewise 6 for `text-scale-decrease`

#### Scenario: The workspace modeline follows
- **GIVEN** the font size is 14 and `workspace_font_size` is 13
- **WHEN** `text-scale-increase` runs
- **THEN** the workspace modeline is drawn at 14 px
