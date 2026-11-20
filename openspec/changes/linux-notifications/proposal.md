# Proposal

## Why

On Linux, `Effect::Notify` is dropped, so sessions and long terminal commands can't reach you while you look at another app (#527).

## What Changes

- The Linux frontend posts notifications over the XDG notifications D-Bus interface through `notify-rust`, as Zed's `gpui_linux` does: app name `aiditor`, icon `aiditor` and the `desktop-entry` hint `aiditor`, so the desktop shows the installed entry's name and icon.
- A newer notification with the same tag replaces the older one (`replaces_id`).
- Clicking a notification (its default action) sends `Msg::NotificationClicked`, and the frontend asks the window manager to raise the window.
- When the post fails (no session bus or no notification server), `*Messages*` gets `Notification not posted: <reason>` once; nothing else fails.
- `notify-rust` is a Linux-only dependency, so macOS doesn't build it.

Not included: action buttons, dismissing, an activation token from the server (GPUI 0.2.2 can't take one).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `system-notifications`: Linux posts too; a click raises the window.

## Impact

- `crates/aiditor`: a Linux `notifications` module, the effect's handling and the raise on click; `Cargo.toml` gains `notify-rust` ~4.18 for Linux (zbus 5, already in the tree through GPUI).
- `docs/editor.md`: the notifications entry records what was built.
