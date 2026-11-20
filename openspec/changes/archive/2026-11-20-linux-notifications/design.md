# Design

## Context

The macOS module (`crates/aiditor/src/notifications.rs`) gives `Notifications::new(tx)` and `show(tag, title, body, tx)`; `Editor` keeps it in a `OnceCell<Option<_>>`. Zed's `gpui_linux/src/linux/system_notifications.rs` posts with `notify-rust` on a thread per notification that blocks in `wait_for_action`.

## Decisions

- **Same interface, per-platform file.** `notifications.rs` stays the macOS module; a `#[path]` Linux module `notifications_linux.rs` exposes the same `Notifications::new`/`show`, so `Editor::handle` drops its cfg split to `any(macos, linux)`. `new` always succeeds on Linux.
- **Thread per notification**, as Zed: `show` connects to the session bus and `wait_for_action` blocks until the notification closes. The `default` action sends `Msg::NotificationClicked { tag }`; `__closed` does nothing.
- **Replacement.** A `Mutex<HashMap<tag, server id>>` records each post's id; the next post with that tag passes it as `replaces_id`. The older thread may also see the click; a second `NotificationClicked` is idempotent.
- **Name and icon.** `appname("aiditor")`, `icon("aiditor")` and `Hint::DesktopEntry("aiditor")`, matching `aiditor.desktop` and the hicolor icon `scripts/bundle.sh` installs. Unbundled runs get the name; the icon only resolves once installed.
- **Failures.** The first failed `show` sends `Msg::Log("Notification not posted: <error>")`, through a static flag as on macOS; later posts are tried again silently, so a server started later works.
- **Raise on click.** `Editor::handle` sees `Msg::NotificationClicked` and defers `window.activate_window()` for every window. On X11 that raises it; on Wayland GPUI asks for an activation token without the server's, so KWin may only mark the window as wanting attention. Applied on macOS too, where the OS already raised it.

## Risks / Trade-offs

- [Wayland focus-stealing prevention] → checked by hand on Fedora KDE; recorded as a limit if it only highlights.
- [One blocked thread per live notification] → as Zed; they end when the notification closes.
