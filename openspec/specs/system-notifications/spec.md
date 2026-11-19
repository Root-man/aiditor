# system-notifications Specification

## Purpose
System notifications tell you about a buffer that needs you while you look at another app, and a click on one brings you to that buffer.

## Requirements

### Requirement: Notifications are posted on macOS from the app bundle
The core SHALL ask for a notification with a tag naming its buffer, a title and a body, and the macOS frontend SHALL post it as a system notification attributed to aiditor when it runs from `aiditor.app`. The first notification SHALL ask for authorization; nothing SHALL ask before then. A newer notification with the same tag SHALL replace the older one. The notification SHALL show even when aiditor is the frontmost app. The runner, the headless binary and the Linux frontend SHALL ignore the request.

#### Scenario: First notification asks for authorization
- **GIVEN** aiditor runs from `aiditor.app` and has never posted a notification
- **WHEN** a notification is requested
- **THEN** macOS asks whether aiditor may send notifications, and once allowed the notification appears with its title and body

#### Scenario: Headless ignores the request
- **WHEN** a key script makes the core request a notification in `aiditor-headless`
- **THEN** nothing is posted and the script goes on

### Requirement: Unbundled runs and failures are noted once
When the binary runs without a main bundle identifier, as `aiditor .` and `cargo run` do, the frontend SHALL post nothing and SHALL append `Notifications need aiditor.app` to `*Messages*` the first time a notification is requested, without aborting. A denied authorization or a failed post SHALL be appended to `*Messages*` once per run.

#### Scenario: Run outside the bundle
- **GIVEN** aiditor runs as `cargo run -p aiditor`
- **WHEN** two notifications are requested
- **THEN** the process keeps running and `*Messages*` holds `Notifications need aiditor.app` once

### Requirement: The notifications setting turns them off
Settings SHALL accept a boolean `notifications`, bundled as `true`. With `false`, the core SHALL request no notification. Other types SHALL reject the settings through the existing error flow.

#### Scenario: Turned off
- **GIVEN** `settings.toml` sets `notifications = false`
- **WHEN** `M-x notify-test` runs
- **THEN** no notification is requested

### Requirement: The core knows whether the window is active
The frontend SHALL tell the core whenever its window becomes active or inactive, and the core SHALL start out treating the window as active.

#### Scenario: Window deactivates
- **WHEN** the frontend reports the window inactive, then active
- **THEN** the core treats the window as inactive, then active

### Requirement: Clicking a notification shows its buffer
Clicking a notification SHALL bring aiditor to the front and show the notification's buffer in the selected window, switching first to the workspace that owns the buffer when it isn't the current one. A buffer that no longer exists SHALL do nothing.

#### Scenario: Buffer in another workspace
- **GIVEN** a notification for buffer `b` in workspace `one` while workspace `two` is current
- **WHEN** the notification is clicked
- **THEN** `one` is the current workspace and its selected window shows `b`

#### Scenario: Killed buffer
- **GIVEN** a notification for a buffer that has since been killed
- **WHEN** the notification is clicked
- **THEN** the workspace and windows are unchanged

### Requirement: notify-test posts a notification for the current buffer
`M-x notify-test` SHALL request a notification tagged with the current buffer, titled `aiditor` with the body `Test notification for <buffer name>`, subject to the `notifications` setting.

#### Scenario: Test notification
- **GIVEN** the current buffer is `notes.txt`
- **WHEN** `M-x notify-test` runs
- **THEN** a notification titled `aiditor` with the body `Test notification for notes.txt` is requested, tagged with that buffer

### Requirement: Agent session events notify while the session can't be seen
A session SHALL request a notification tagged with its session buffer and titled `<agent> · <workspace>`, with the names its buffer was given, when it asks for permission (body: the tool call's title), asks a blocking question (body: the question), ends a turn not cancelled (body: `Turn ended` for `end_turn`, otherwise the line the session buffer shows for the end), or fails to start or exits on its own (body: the line `*Messages*` or the echo area gets). A cancelled turn and a non-blocking question SHALL request none. A notification SHALL be requested only while the window is inactive or the current workspace's windows don't show the session buffer.

#### Scenario: Permission while the window is inactive
- **GIVEN** a ready `claude` session in workspace `work`, shown, and the window inactive
- **WHEN** the agent asks permission for tool call `Edit main.rs`
- **THEN** a notification titled `claude · work` with body `Edit main.rs` is requested, tagged with the session buffer

#### Scenario: Seen session
- **GIVEN** the session buffer is shown in the current workspace and the window is active
- **WHEN** the agent asks permission, asks a question, ends a turn or exits
- **THEN** no notification is requested

#### Scenario: Session in another workspace
- **GIVEN** the window is active and another workspace is current
- **WHEN** the session's turn ends with `end_turn`
- **THEN** a notification with body `Turn ended` is requested

#### Scenario: Cancelled turn
- **GIVEN** the window is inactive
- **WHEN** the turn ends with `cancelled`
- **THEN** no notification is requested

#### Scenario: Crash
- **GIVEN** the window is inactive
- **WHEN** the agent exits with code 1
- **THEN** a notification with body `claude session exited with code 1` is requested

#### Scenario: Later events replace earlier ones
- **WHEN** the session asks a question and then ends its turn while unseen
- **THEN** both notifications carry the same tag

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
