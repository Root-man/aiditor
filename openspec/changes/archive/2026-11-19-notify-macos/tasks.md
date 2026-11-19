# Tasks

## 1. Core

- [x] 1.1 Add `Effect::Notify { tag, title, body }`, `Msg::WindowActive(bool)` and `Msg::NotificationClicked { tag }`; pass the effect through the runner as `Run::Platform` and ignore it in the headless binary and service test support; verify `cargo build --workspace`
- [x] 1.2 Add the `notifications` setting (bundled `true`) and `Ctx::notify`; verify a core test that `notifications = false` requests nothing
- [x] 1.3 Store `window_active` from `Msg::WindowActive`; verify a core test
- [x] 1.4 Route `Msg::NotificationClicked` to the buffer's workspace and buffer; verify core tests for another workspace, a killed buffer and a bad tag
- [x] 1.5 Add `notify-test`; verify a core test of the requested effect

## 2. Frontend

- [x] 2.1 Add the macOS-only dependencies and `notifications.rs`, trimmed from Zed's; verify `cargo build -p aiditor` on macOS
- [x] 2.2 Carry out `Effect::Notify` in `Editor::handle` (dropped off macOS) and send `Msg::WindowActive` from `observe_window_activation`; verify `cargo test -p aiditor`
- [x] 2.3 Check by hand: unbundled run notes once without aborting; from `aiditor.app` a notification posts, asks authorization, and clicking it shows the buffer

## 3. Docs

- [x] 3.1 Update the notifications entry in `docs/editor.md` with what was built
