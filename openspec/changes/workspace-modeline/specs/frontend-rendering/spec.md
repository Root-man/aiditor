## MODIFIED Requirements

### Requirement: The workspace modeline is drawn above every window
The frontend SHALL draw one workspace modeline row above the window tree, its `workspace-bar` background spanning the frame edge to edge while its text keeps the 12 px inset, in the editor's font at the workspace font size and that size's line height plus 4 px, rounded. It SHALL draw the core's fitted workspace spans with their faces and the local 24-hour `HH:MM` clock at the right inset, leaving at least two columns between groups when space permits. The windows SHALL be laid out below it in windowed and fullscreen frames alike.

#### Scenario: One bar above the splits
- **GIVEN** workspace `main` of `home` and workspace `work` of `aiditor`
- **WHEN** the frame shows two side-by-side windows and has room for both entries
- **THEN** one row reading `home › main │ aiditor › work` is drawn above both windows, with a right-aligned clock, and the windows start below it

#### Scenario: Design size
- **GIVEN** `workspace_font_size` is 13 and `line_height` is 1.4
- **WHEN** the bar is drawn
- **THEN** it is 22 px high

## ADDED Requirements

### Requirement: The workspace clock updates on minute boundaries
The clock SHALL show the local hour and minute at launch, with leading zeroes, and refresh on the next minute boundary even without editor input. Each wakeup SHALL read local wall time and recalculate the next boundary, so late wakeups, day rollover and clock changes do not accumulate drift. Clock reads SHALL occur outside the core.

#### Scenario: A quiet minute rolls over
- **GIVEN** the clock reads `20:41` at 20:41:59.750
- **WHEN** the next 250 ms pass without input
- **THEN** the frontend refreshes to `20:42` and schedules the next minute boundary

#### Scenario: Day rollover
- **GIVEN** local time is 23:59:59
- **WHEN** the next minute arrives
- **THEN** the clock shows `00:00`
