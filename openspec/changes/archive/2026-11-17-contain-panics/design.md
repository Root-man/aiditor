# Design

## Context

`Ctx::try_call` chains a panic hook once that stores each panic's report and backtrace in a thread local, runs the handler under `catch_unwind`, then echoes and logs. Everything the frontend asks the core goes through `Editor::handle` (`Core::handle`) except redisplay, which calls `Core::frame` from `render` and `FrameElement::prepaint`.

## Goals / Non-Goals

**Goals:** contain panics in `Core::handle` and `Core::frame` as called by the frontend, reusing #52's hook and report.

**Non-Goals:**
- Trivial accessors (`settings`, `point`, `current`, `next_deadline`, `wants_clipboard`): field reads and lookups, not worth wrapping.
- Panics in the services runner or GPUI itself: they are not calls into the core.
- Startup calls in `main` (`launch`, `open`, the first messages): they run before the window and outside any platform callback, so a panic there ends the launch as before.
- Containing inside `Core::handle` itself: tests that drive the core directly should still see panics.

## Decisions

- **One helper, `Core::contain(what, f) -> Option<R>`.** It installs the hook, runs `f(&mut Core)` under `catch_unwind`, and on a panic echoes `<what> panicked; its state may be inconsistent (see *Messages*)` and logs the line plus report. `try_call` keeps its own `catch_unwind` because its closure borrows the `Ctx`, but shares a private `caught` function that installs the hook and takes the report. Alternative: a free function returning the report; the method keeps echo/log private to the core.
- **Labels.** `The core` for message handling (a command's own panic is already caught and named inside), `Redisplay` for the frame.
- **A failed frame draws as an empty `FrameDisplay` with the core's message.** `FrameDisplay` derives `Default`. The echo line still shows, so `C-x C-s` and `C-x C-c` work blind.
- **A failed `handle` returns no effects**, so the timer is re-armed from the core's current timers and the editor carries on.

## Risks / Trade-offs

- [A persistent panic in `frame` repeats on each redisplay] → each is logged; redisplay only happens after input or a message, so `*Messages*` grows per event, not per vsync.
- [Effects produced before a panic in `handle` are lost] → same inconsistency the echo line warns about.
