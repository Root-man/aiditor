# Spec Delta

## ADDED Requirements

### Requirement: Line height and workspace font size settings
`line_height` SHALL be a multiple of the font size from 1.0 to 3.0, bundled as 1.4. `workspace_font_size` SHALL be the workspace modeline's size from 6 to 72, bundled as 13. `C-x C-r` SHALL apply both, and a value out of range SHALL reject the whole file.

#### Scenario: Applied on reload
- **WHEN** the user file sets `line_height = 1.6` and `workspace_font_size = 12` and `C-x C-r` reloads it
- **THEN** lines are 1.6 font sizes high and the workspace modeline is drawn at 12 px

#### Scenario: Out of range
- **WHEN** the user file sets `line_height = 0.5` or `workspace_font_size = 100` and the settings are reloaded
- **THEN** an error is shown and the previous settings stay in effect
