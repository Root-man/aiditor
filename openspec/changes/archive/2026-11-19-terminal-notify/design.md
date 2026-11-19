# Design

## Context

`Terminal::feed` already parses OSC 133 `C` (with `cmdline_url=`) and `D` (with the exit code), tracking `running` and `failed` for the mode line. The core feeds output in `dispatch`'s `Msg::TerminalOutput` arm, which knows the handle time `now` (a fake clock in the harness). `Ctx::notify_unseen` (#525) posts unless the window is active and the current workspace shows the buffer.

## Goals / Non-Goals

**Goals:** one notification per long command, timed from the batch carrying `C` to the batch carrying `D`, through the core's clock so tests use the fake one.

**Non-Goals:** notifying on shell exit, shells without OSC 133, withdrawing notifications.

## Decisions

- **The terminal times commands.** `Terminal::feed_at(bytes, now)` keeps `began: Option<(Instant, Option<String>)>`, set at `C`, cleared at `A` (a shell that sends no `D`), and taken at `D`, and returns the replies plus the finished command (`Finished`: command line, exit code, duration) if one ended in the batch. `feed(bytes)` stays for benches and tests, timed with `Instant::now()`. `running` can't be reused: it is `None` for a `C` without a command line, and is the mode line's.
- **The core decides.** The `TerminalOutput` arm compares the duration with `terminal_notify_after` (0 off) and calls `notify_unseen` on the terminal buffer.
- **Text.** Title: the command line with control chars as spaces, as the mode line does, else `Command`. Body: `finished in <d>` for no code or 0, else `failed (exit <n>) after <d>`, `<d>` from `long_duration` (`12s`, `2m 05s`, `1h 02m 05s`), shared with sessions' timers.
- **Threshold.** A command notifies when its duration is at least the setting's seconds.

## Risks / Trade-offs

- [Output batches are timed when the core handles them, not when the shell wrote the marks] → seconds of precision are enough for a 10 s threshold.
- [Two commands ending in one batch report only the last] → each needs typed input between them, so both can't be long in one batch.
