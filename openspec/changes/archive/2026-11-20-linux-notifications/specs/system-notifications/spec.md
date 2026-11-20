# Spec Delta

## MODIFIED Requirements

### Requirement: Notifications are posted on macOS from the app bundle
The core SHALL ask for a notification with a tag naming its buffer, a title and a body, and the macOS frontend SHALL post it as a system notification attributed to aiditor when it runs from `aiditor.app`. The first notification SHALL ask for authorization; nothing SHALL ask before then. A newer notification with the same tag SHALL replace the older one. The notification SHALL show even when aiditor is the frontmost app. The runner and the headless binary SHALL ignore the request.

#### Scenario: First notification asks for authorization
- **GIVEN** aiditor runs from `aiditor.app` and has never posted a notification
- **WHEN** a notification is requested
- **THEN** macOS asks whether aiditor may send notifications, and once allowed the notification appears with its title and body

#### Scenario: Headless ignores the request
- **WHEN** a key script makes the core request a notification in `aiditor-headless`
- **THEN** nothing is posted and the script goes on

### Requirement: Clicking a notification shows its buffer
Clicking a notification SHALL bring aiditor to the front and show the notification's buffer in the selected window, switching first to the workspace that owns the buffer when it isn't the current one. A buffer that no longer exists SHALL leave the workspace and windows unchanged. On Linux the frontend SHALL ask the window manager to raise the window, which a Wayland compositor may answer by only marking it as wanting attention.

#### Scenario: Buffer in another workspace
- **GIVEN** a notification for buffer `b` in workspace `one` while workspace `two` is current
- **WHEN** the notification is clicked
- **THEN** `one` is the current workspace and its selected window shows `b`

#### Scenario: Killed buffer
- **GIVEN** a notification for a buffer that has since been killed
- **WHEN** the notification is clicked
- **THEN** the workspace and windows are unchanged

## ADDED Requirements

### Requirement: Notifications are posted on Linux over D-Bus
The Linux frontend SHALL post each requested notification over the XDG notifications D-Bus interface with app name `aiditor`, icon `aiditor` and desktop entry `aiditor`. A newer notification with the same tag SHALL replace the older one. Activating the notification's body SHALL count as clicking it. The macOS build SHALL NOT depend on `notify-rust`.

#### Scenario: Posted on KDE
- **GIVEN** aiditor runs on Fedora KDE and the window is inactive
- **WHEN** a session's turn ends
- **THEN** a notification named aiditor with its icon shows the title and body

### Requirement: Failed Linux posts are noted once
When a post fails on Linux, as without a session bus or a notification server, the frontend SHALL append `Notification not posted: <reason>` to `*Messages*` once per run and SHALL keep running.

#### Scenario: No notification server
- **GIVEN** the session bus has no `org.freedesktop.Notifications`
- **WHEN** two notifications are requested
- **THEN** the process keeps running and `*Messages*` holds one `Notification not posted: …` line
