# Tasks

## 1. Core containment helper

- [x] 1.1 Add `Core::contain` sharing the panic hook and report with `Ctx::try_call`; derive `Default` on `FrameDisplay`; verify with a core unit test that a panicking closure returns `None`, echoes `<what> panicked; …` and logs the panic and backtrace, and the existing command panic test still passes

## 2. Frontend wrapping

- [x] 2.1 Run `Core::handle` in `Editor::handle` through `contain("The core", …)` and verify with a GPUI test that a panicking message (a timer's wake-up) leaves the editor running, reports in `*Messages*`, and a later key is inserted
- [x] 2.2 Build the frame through `contain("Redisplay", …)` in `render` and `prepaint`, falling back to an empty frame with the core's message; verify with a GPUI test that a panicking frame build draws and keeps handling keys
- [x] 2.3 Update `docs/editor.md` "Contain panics" to record the frontend wrapper and drop its "Not yet"; verify the text

## Workflow follow-up

- Archive the change after review.
