# Design

## Context

Platform effects (`SetClipboard`, `OpenUrl`, `OpenWithSystem`, `Quit`) leave the runner as `Run::Platform` and are carried out in `Editor::handle`. Services answer the core over the `Sender<Msg>` the editor drains. `objc2` 0.6, `objc2-foundation` 0.3.2 and `block2` 0.6 are already in `Cargo.lock`; `objc2-user-notifications` is not.

## Goals / Non-Goals

**Goals:** a post path the event tasks can call with one line, and click routing tested in the core.

**Non-Goals:** deciding when to notify (#525, #526), Linux (#527), actions and dismissal.

## Decisions

- **Tag is the buffer number as text.** `Effect::Notify { tag, title, body }` and `Msg::NotificationClicked { tag }` carry a `String`, as the issue names them; the core formats `BufferId::number()` and parses it back. A tag that doesn't parse or names no buffer does nothing. Alternative: carry `BufferId` and convert in the frontend; it would need a public constructor for a core id.
- **One core helper.** `Ctx::notify(buffer, title, body)` checks the `notifications` setting and pushes the effect. `notify-test` uses it; #525/#526 will too and add their own visibility checks.
- **Click routing.** Look the buffer up; if it has a workspace that isn't current, `switch_workspace`, then `switch_to(buffer)`. A buffer with no workspace (`*Messages*`) is shown in the current one.
- **Raising the app.** macOS activates the app when the user clicks a notification's default action, so the frontend does nothing extra. Checked by hand.
- **Frontend module.** `crates/aiditor/src/notifications.rs`, `#[cfg(target_os = "macos")]`, a trimmed copy of Zed's `gpui_macos/src/system_notifications.rs`: no categories or actions, no dismiss, no foreground executor. The delegate and completion handlers run on arbitrary threads and `try_send` `Msg::NotificationClicked` or `Msg::Log` into the services channel the editor already drains. The center is created lazily on the first `Effect::Notify` and kept on the `Editor`; a missing bundle identifier makes it `None` after one `Msg::Log`. Post failures are logged once through a static flag. Other platforms drop the effect.
- **Focus.** `cx.observe_window_activation` in `Editor::new` sends `Msg::WindowActive(window.is_window_active())`; the core stores `window_active`, starting `true`. Nothing reads it yet besides tests.

## Risks / Trade-offs

- [Ad-hoc signed bundle may not be allowed to post] → checked by hand from `aiditor.app`; if macOS refuses, the failure is noted in `*Messages*`.
- [Channel full drops a click] → the channel holds 256 messages and clicks are rare; accepted.
