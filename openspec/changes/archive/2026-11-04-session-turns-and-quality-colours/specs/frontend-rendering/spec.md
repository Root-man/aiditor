## MODIFIED Requirements

### Requirement: The core gets the wall clock from the frontend
The frontend SHALL give the core the local time of day, to the nanosecond, at startup and at each minute wakeup, so the core never reads the wall clock. Between wakeups the core SHALL count on from the last time given by its own monotonic time, wrapping at midnight. The workspace clock SHALL keep showing `HH:MM`.

#### Scenario: A minute passes
- **WHEN** the workspace clock changes from `20:41` to `20:42`
- **THEN** a message stamped afterwards shows a time from `20:42:00`

#### Scenario: Midnight
- **WHEN** the frontend last gave 23:59:58 and a message is stamped 3s later
- **THEN** it shows `00:00:01`

## ADDED Requirements

### Requirement: Borders and horizontal rules are drawn
A line's top or bottom border SHALL be drawn as a 1 px line in its face's foreground across the window's width, inside the line's row, at its top or bottom edge, over any background. A line's horizontal rule SHALL be drawn as a 1 px line in its face's foreground through the middle of the row, in every stretch of the window's width that holds no glyph, kept one cell clear of the glyphs on each side.

#### Scenario: A prompt's borders
- **WHEN** a session echo of two lines is drawn
- **THEN** a 1 px `session-user-rule` line runs across the window at the top of its first row and the bottom of its second

#### Scenario: A settled turn
- **WHEN** `Worked 2m 05s` with its time `13:58:12` is drawn as a rule
- **THEN** a 1 px `separator` line runs through the middle of the row before the label, between the label and the time and after the time, a cell clear of each
