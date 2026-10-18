# settings Specification

## Purpose

Loading the editor's TOML settings from bundled defaults and the user's config dir, validating them, reporting errors and reloading them at runtime.

## Requirements

### Requirement: User settings override bundled defaults
The editor SHALL ship bundled default settings and SHALL read the user's `settings.toml` from the platform config dir, with each key the user sets overriding the default and every other key keeping its default. A missing user file SHALL mean the defaults alone.

#### Scenario: Partial user file
- **WHEN** the user file sets only the font size
- **THEN** the font size is the user's and the font family and theme are the defaults

#### Scenario: No user file
- **WHEN** no user settings file exists
- **THEN** the bundled defaults apply and no error is reported

### Requirement: M0 settings cover font and theme
The settings SHALL hold a font family, a font size, a default theme of background, foreground, cursor, region, mode line and inactive mode line colors, and `option_as_meta`, and the frame SHALL draw with them. An unset font family SHALL mean the platform's monospace font. `option_as_meta` SHALL default to true and SHALL only matter on macOS.

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

### Requirement: Invalid settings are rejected whole
A settings file that is not valid TOML, has a value of the wrong type, a font size outside 6 to 72, or a key the editor does not know SHALL be rejected. The editor SHALL show an error naming the problem and SHALL keep the settings it had before.

#### Scenario: Wrong type
- **WHEN** the user file sets the font size to `"big"` and the settings are reloaded
- **THEN** an error is shown and the previous font size stays in effect

#### Scenario: Unknown key
- **WHEN** the user file sets `font_sise = 16`
- **THEN** an error naming the unknown key is shown and the previous settings stay in effect

#### Scenario: Out of range
- **WHEN** the user file sets the font size to 0
- **THEN** an error is shown and the previous font size stays in effect

#### Scenario: Unreadable file
- **WHEN** the user file exists but cannot be read
- **THEN** an error is shown and the previous settings stay in effect

### Requirement: C-x C-r reloads settings
`C-x C-r` SHALL read the user settings file again and apply it, and the frame SHALL redraw with the new values.

#### Scenario: Font size change takes effect
- **WHEN** the user file's font size is changed from 14 to 18 and `C-x C-r` is pressed
- **THEN** the font size in effect is 18

### Requirement: Settings errors stay visible until the next key
A settings error SHALL be shown on one line at the bottom of the frame and SHALL clear on the next key.

#### Scenario: Cleared by a key
- **WHEN** a settings error is shown and any key is pressed
- **THEN** the error line is gone
