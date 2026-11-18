# Proposal

## Why

aiditor can't tell you that a session or a terminal needs you while you look at another app: GPUI 0.2.2 has no notification API. This change lays the macOS plumbing the event tasks (#525, #526) build on.

## What Changes

- A new core effect asks the frontend to post a system notification with a tag, a title and a body; the runner and headless binaries ignore it.
- The macOS frontend posts it through `UNUserNotificationCenter`, from a trimmed copy of Zed's `gpui_macos` notification code: no action buttons, one click callback. The center is touched on the first notification, which asks for authorization the first time.
- Run outside `aiditor.app` (no main bundle identifier), the frontend posts nothing and notes once in `*Messages*` that notifications need the app bundle. A denied authorization or a failed post is noted once too.
- The frontend tells the core when its window becomes active or inactive.
- Clicking a notification shows its buffer: the core switches to the buffer's workspace and shows the buffer there. A buffer that is gone does nothing.
- A new `notifications` setting, bundled as `true`; `false` posts nothing.
- A new `notify-test` command posts a notification for the current buffer, to check the setup by hand.

Not included: the events that notify (#525, #526), the "only when you can't see it" rule (left to those tasks), Linux (#527, where the effect is ignored for now), action buttons and dismissing notifications.

## Capabilities

### New Capabilities

- `system-notifications`: posting a notification, the unbundled and failure notes, the setting, clicks, and `notify-test`.

### Modified Capabilities

None.

## Impact

- `crates/aiditor-core`: `Effect::Notify`, `Msg::WindowActive`, `Msg::NotificationClicked`, the setting, the command and the click routing; `settings.toml`.
- `crates/aiditor-services`: the runner, headless binary and test support pass the effect on or ignore it.
- `crates/aiditor`: a macOS-only notifications module, the effect's handling and the window activation observer.
- New macOS-only dependencies of the frontend: `objc2-user-notifications` 0.3, plus `objc2`, `objc2-foundation` and `block2`, already in the tree through GPUI.
- `docs/editor.md`: the notifications entry records what was built.
