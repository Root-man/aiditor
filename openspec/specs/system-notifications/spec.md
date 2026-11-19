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
