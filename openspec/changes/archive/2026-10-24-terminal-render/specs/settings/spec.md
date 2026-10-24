# Spec Delta

## MODIFIED Requirements

### Requirement: M0 settings cover font and theme
The settings SHALL hold a font family, a font size, a default theme of background, foreground, cursor, region, mode line and inactive mode line colors and a 16-colour ANSI palette, and `option_as_meta`, and the frame SHALL draw with them. An unset font family SHALL mean the platform's monospace font. `option_as_meta` SHALL default to true and SHALL only matter on macOS.

#### Scenario: Font size drawn
- **WHEN** the font size setting is 18
- **THEN** buffer text is drawn at 18 px

#### Scenario: Region color
- **WHEN** the user file sets the theme's region color
- **THEN** the display model highlights the region in that color

#### Scenario: Mode line colors
- **WHEN** the user file sets the theme's `mode_line` color
- **THEN** the selected window's mode line is drawn on it and the other mode lines keep the default inactive color

#### Scenario: Option as Meta by default
- **WHEN** no user file sets `option_as_meta`
- **THEN** `option_as_meta` is true

#### Scenario: ANSI palette
- **WHEN** the user file sets `theme.ansi` to 16 colours
- **THEN** terminal cells in ANSI colour 1 use the user's second colour
