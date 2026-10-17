# Spec Delta

## MODIFIED Requirements

### Requirement: M0 settings cover font and theme
The settings SHALL hold a font family, a font size and a default theme of background, foreground, cursor and region colors, and the frame SHALL draw with them. An unset font family SHALL mean the platform's monospace font.

#### Scenario: Font size drawn
- **WHEN** the font size setting is 18
- **THEN** buffer text is drawn at 18 px

#### Scenario: Region color
- **WHEN** the user file sets the theme's region color
- **THEN** the display model highlights the region in that color
