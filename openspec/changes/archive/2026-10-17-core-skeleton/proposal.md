# Proposal

## Why

`Core::handle(&mut self, Msg)` returns nothing, the Tokio runtime in `main.rs` runs no services, and nothing delivers their results or any timer to the core. The walking skeleton (#10), the command registry (#20, which needs `Ctx`), auto-save, the which-key hint and file I/O are all blocked on that loop. This is issue [#19](https://github.com/Root-man/aiditor/issues/19).

## What Changes

- `Core::handle(&mut self, now: Instant, msg: Msg) -> Effects`. Every message is stamped with the time it is handled, so the core never reads the clock and tests drive a fake one. **BREAKING** for the one caller (the harness).
- `Effect` enum and `Effects = Vec<Effect>`: requests the app carries out for the core. It starts empty; file I/O and later services add variants.
- `Ctx`: what a command handler gets. It has the core, the current time and the effects being collected.
- Timers owned by the core: one-shot and repeating, cancellable, each carrying the `Msg` it delivers when it fires. Due timers fire on every `handle` call; `Msg::Tick` is just a wake-up. `Core::next_deadline()` tells the app when to wake the core next.
- Service bridge in the app: a bounded `tokio::sync::mpsc` channel of `Msg`, whose sender services get. A GPUI foreground task drains it into the core. After each `handle`, the app re-arms one GPUI timer to `next_deadline()`.
- The test harness gets a fake clock: `advance(Duration)` moves time on and sends `Msg::Tick`.

Out of scope: key input in the GPUI view (#10), real services and their request effects (file I/O and later), the command registry (#20), catching panics around `handle` (#52), and channel backpressure tests.

## Capabilities

### New Capabilities

- `core-loop`: how messages reach the core and what comes back: `handle` with a timestamp, effects, the command context, and service results delivered through a bounded channel.
- `timers`: one-shot and repeating timers that the core owns, cancellation, firing as messages, and the next deadline the app wakes on.

### Modified Capabilities

None. `text-buffers` requirements do not change.

## Impact

- `crates/aiditor-core`: `lib.rs` (`Msg`, `Effect`, `Ctx`, the new `handle`), a new `timer` module, `harness.rs` (fake clock).
- `crates/aiditor-services`: depends on `aiditor-core` for `Msg`, and exposes the channel constructor; holds the bridge test.
- `crates/aiditor/src/main.rs`: the channel, the drain task and timer re-arming in the `Editor` view.
- `docs/editor.md`: the core-thread section records core-owned timers and the timestamped `handle`.
- No new dependencies. The core still has no GPUI or Tokio dependency (CI checks this).
