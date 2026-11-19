## ADDED Requirements

### Requirement: Long terminal commands notify while their terminal can't be seen
A terminal command SHALL request a notification tagged with its terminal buffer when its OSC 133 `D` mark arrives at least `terminal_notify_after` seconds after its `C` mark, measured on the core's clock, and the setting is not 0. The title SHALL be the command line the `C` mark gave, control characters shown as spaces, or `Command` without one. The body SHALL be `finished in <duration>` when the exit code is 0 or absent, otherwise `failed (exit <code>) after <duration>`, the duration as `12s`, `2m 05s` or `1h 02m 05s`. A prompt mark between them SHALL drop the command. A notification SHALL be requested only while the window is inactive or the current workspace's windows don't show the terminal buffer.

#### Scenario: Long command while the window is inactive
- **GIVEN** a terminal and the window inactive
- **WHEN** the shell marks `C` with command line `sleep 12`, then `D;0` 12 seconds later
- **THEN** a notification titled `sleep 12` with body `finished in 12s` is requested, tagged with the terminal buffer

#### Scenario: Failing command
- **GIVEN** the window is inactive
- **WHEN** a command without a command line ends with `D;2` after 2 minutes 16 seconds
- **THEN** a notification titled `Command` with body `failed (exit 2) after 2m 16s` is requested

#### Scenario: Short command
- **WHEN** a command ends 9 seconds after it started, with the setting at 10
- **THEN** no notification is requested

#### Scenario: Shown terminal
- **GIVEN** the window is active and the current workspace shows the terminal
- **WHEN** a long command ends
- **THEN** no notification is requested

#### Scenario: Turned off
- **GIVEN** `terminal_notify_after = 0`
- **WHEN** a long command ends while the window is inactive
- **THEN** no notification is requested
