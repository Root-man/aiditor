# Tasks

## 1. Frontend

- [x] 1.1 Add `notify-rust` ~4.18 as a Linux-only dependency; verify `cargo tree` shows one zbus and no notify-rust for macOS targets
- [x] 1.2 Add the Linux `Notifications` module (post, replace by tag, default action click, failure noted once); verify a unit test of the failure note
- [x] 1.3 Carry out `Effect::Notify` on Linux and raise the window on `Msg::NotificationClicked`; verify `cargo test -p aiditor`
- [ ] 1.4 Check on Fedora KDE: a notification posts named aiditor with its icon, replaces by tag, a click shows the buffer and raises the window; without a server `*Messages*` notes it once

## 2. Docs

- [x] 2.1 Update the notifications entry in `docs/editor.md` with what was built
