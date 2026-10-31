# Spec Delta

## Purpose

Marks point's line with the `hl-line` background, as Emacs's `hl-line-mode`, so point can be found at a glance.

## ADDED Requirements

### Requirement: Point's line has the hl-line background
Every display row of the buffer line holding point SHALL carry the `hl-line` face's background across the window's width, gutter included, in file buffers, `*scratch*`, `*Messages*` and terminal copy mode. Other buffers SHALL show none.

#### Scenario: A file buffer
- **WHEN** a file is shown with point on its second line
- **THEN** that line has the `hl-line` background and the others do not

#### Scenario: A wrapped line
- **WHEN** point's line wraps into several rows
- **THEN** each of those rows has the background

#### Scenario: Terminal copy mode
- **WHEN** a terminal is in copy mode
- **THEN** point's line has the background, and in char mode nothing does

#### Scenario: Another buffer
- **WHEN** a buffer that visits no file, made with `C-x b`, is shown
- **THEN** no line has the background

### Requirement: One window shows it
Only the selected window SHALL show the background. While the minibuffer is open, the window it was opened from SHALL show it, and the minibuffer's own line SHALL not.

#### Scenario: Two windows on one buffer
- **WHEN** two windows show the same file
- **THEN** only the selected window's point's line has the background

#### Scenario: A preview
- **WHEN** a minibuffer previews a position in the window it was opened from
- **THEN** that window's point's line has the background

### Requirement: Other backgrounds draw over it
The region and any other face with a background SHALL draw over `hl-line` where they overlap, and the rest of the line SHALL keep it.

#### Scenario: The region on point's line
- **WHEN** the region covers part of point's line
- **THEN** those chars have the `region` background over the line's `hl-line` background

#### Scenario: A match on point's line
- **WHEN** a text property with a background covers part of point's line
- **THEN** those chars have that background over the line's `hl-line` background

### Requirement: The hl_line setting turns it off
The `hl_line` setting SHALL be a boolean, bundled as `true`; `false` SHALL show no background in any window, a reloaded value included. Any other value SHALL be rejected as an invalid setting.

#### Scenario: Turned off
- **WHEN** `hl_line = false` is loaded
- **THEN** no line has the background

#### Scenario: A bad value
- **WHEN** the user file sets `hl_line = "yes"`
- **THEN** an error is shown and the previous settings stay in effect
