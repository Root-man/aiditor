# text-scale Specification

## Purpose
Lets the user change the frame's text size from the keyboard while the editor runs, without editing the settings file.

## Requirements

### Requirement: Zoom keys change the font size
`text-scale-increase` and `text-scale-decrease` SHALL change the frame's font size by 1px, staying within 6 to 72. Line heights, mode lines, gutter digits, the minibuffer and terminal grids SHALL follow the new size. The text area of each window SHALL be laid out again at the new size.

#### Scenario: Increase and decrease
- **GIVEN** the font size is 14
- **WHEN** `text-scale-increase` runs twice and `text-scale-decrease` runs once
- **THEN** the font size is 15 and the line height follows it

#### Scenario: The range holds
- **GIVEN** the font size is 72
- **WHEN** `text-scale-increase` runs
- **THEN** the font size stays 72, and likewise 6 for `text-scale-decrease`

### Requirement: Reset returns to the settings file's size
`text-scale-reset` SHALL set the font size to the `font_size` the settings give, not to a fixed number.

#### Scenario: Reset to the user's size
- **GIVEN** the settings file sets `font_size = 18`
- **WHEN** `text-scale-increase` and then `text-scale-reset` run
- **THEN** the font size is 18

### Requirement: Zoom survives a reload
Applying settings while zoomed SHALL keep the distance from the settings' `font_size`, within the same range. The zoom SHALL NOT be saved to any file.

#### Scenario: Reload while zoomed
- **GIVEN** the settings give 14 and the font size is 16
- **WHEN** the settings file changes to `font_size = 18` and is reloaded
- **THEN** the font size is 20 and `text-scale-reset` gives 18

### Requirement: The platform's zoom keys are bound
On Linux `C-=` and `C-+` SHALL run `text-scale-increase`, `C--` SHALL run `text-scale-decrease` and `C-0` SHALL run `text-scale-reset`. On macOS `s-=`, `s-+`, `s--` and `s-0` SHALL run them.

#### Scenario: The keys on this platform
- **WHEN** the platform's increase key, decrease key and reset key are pressed in turn
- **THEN** the font size goes up 1px, comes back and stays at the default
