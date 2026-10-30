# Spec Delta

## MODIFIED Requirements

### Requirement: The terminal is sized to its smallest window
A live terminal's grid and PTY SHALL be as many rows as the text area (the window less its top padding and mode line) and as many columns as fit between its 12 px side insets in the narrowest and shortest window of the current workspace showing it, taking rows and columns independently, as Emacs's `window-adjust-process-window-size-smallest`. When that size changes, the core SHALL resize the grid and send one `TerminalOp::Resize`. A terminal no window shows, or one before the frame size is known, SHALL keep its size; a new one starts at 24x80 until the frame size is known.

#### Scenario: Spawned at the window's size
- **WHEN** the window's text area is 80 cells by 24 lines and `M-x vterm` runs in it
- **THEN** the shell is spawned with 24 rows and 80 columns

#### Scenario: Splitting shrinks it
- **WHEN** a terminal is shown in the only window and `C-x 2` splits it
- **THEN** the core resizes the PTY to the smaller of the two windows' text areas

#### Scenario: Resizing the frame
- **WHEN** the frame showing a terminal gets smaller
- **THEN** the core resizes the PTY to the new size, once
