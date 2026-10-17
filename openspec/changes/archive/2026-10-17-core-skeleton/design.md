# Design

## Context

`crates/aiditor-core/src/lib.rs` has `Msg::Key`, `Core { buffers, current, point }` and `Core::handle(&mut self, Msg)` returning `()`, with a hardcoded self-insert. `harness.rs` drives it with key scripts. `main.rs` starts a Tokio runtime, enters it, and opens a GPUI window whose `Editor` view holds a `Core`; nothing is spawned and no key handler exists yet (#10). `aiditor-services` is empty and does not depend on the core. CI fails if `aiditor-core` pulls in `gpui` or `tokio`.

From the GPUI spike (`docs/spikes/gpui.md`): a bounded `tokio::sync::mpsc` channel drained by a `cx.spawn` foreground task works, and the receiver needs no Tokio context. GPUI 0.2.2 has `cx.background_executor().timer(Duration) -> Task<()>`, and dropping a `Task` cancels it.

## Goals / Non-Goals

**Goals:**
- The `Msg -> Effects` loop that #10, #20 and the timer users build on.
- Timer logic (repeat, cancel, ordering) lives where the headless tests run.

**Non-Goals:**
- Any real `Effect` variant or service; the enum starts empty.
- Command lookup: `Ctx` is passed to the existing self-insert only. The registry (#20) adds handlers.
- Testing the GPUI drain task or timer re-arming automatically; they are checked by hand (no screenshot or `TestAppContext` tests, per the spike).

## Decisions

**Time is a parameter of `handle`, not a field of `Msg::Tick`.**

```
pub fn handle(&mut self, now: Instant, msg: Msg) -> Effects
```

A timer started from a key message needs the current time to compute its deadline, and a key message carries none. A `Tick(now)` payload alone would leave the core with a stale "last known time" between ticks. Passing `now` on every call keeps the core clock-free: the app passes `Instant::now()`, and the harness passes its fake clock. `Msg::Tick` stays, without a payload, as the wake-up message the app sends when the deadline passes. A `Clock` trait injected into `Core` was rejected as more code for the same effect.

**Due timers fire on every `handle`, after the message.** Any message is a chance to run overdue timers, so a key press that arrives just after a deadline does not wait for the `Tick`. `Tick` itself dispatches nothing.

**A timer carries the `Msg` it delivers.** `start(after, repeat: Option<Duration>, msg) -> TimerId`. Firing dispatches the message like any other, so auto-save, which-key and debounced grep each add a `Msg` variant, not a callback type. `Msg` derives `Clone` for repeats. Alternatives: firing a bare `Msg::Timer(id)` and making each owner match its stored id (spreads the dispatch around the core), or a `fn(&mut Ctx)` callback (Emacs-like, but it can't be compared or printed in tests and it gets ahead of the registry's handler shape).

**Module `crates/aiditor-core/src/timer.rs`.**

```
TimerId(u64)                         Copy, Eq, Debug; counter only increases
Timers { next: u64, list: Vec<Timer> }
  start(now, after, repeat, msg) -> TimerId
  cancel(id)                         no-op for a gone id
  due(now) -> Vec<TimerId>           snapshot of ids with deadline <= now, in deadline order
  fire(id, now) -> Option<Msg>       None if gone; removes a one-shot, or sets deadline = now + interval
  next_deadline() -> Option<Instant>
Timer { id, deadline: Instant, repeat: Option<Duration>, msg: Msg }
```

`handle` takes the `due` snapshot, then calls `fire` per id and dispatches the result. The snapshot gives both edge cases in the spec: a timer cancelled by an earlier one in the same wake-up returns `None`, and a timer started while firing is not in the snapshot. A `Vec` scanned linearly is enough for the handful of timers an editor runs. A repeat's next deadline is `now + interval`, not `deadline + interval`, so a late wake-up fires once instead of catching up.

**`Ctx` and `Effects`.**

```
pub enum Effect {}                  // variants come with the first service request
pub type Effects = Vec<Effect>;
pub struct Ctx<'a> { pub core: &'a mut Core, pub now: Instant, pub effects: Effects }
  start_timer(after, repeat, msg) -> TimerId,  cancel_timer(id)
```

`handle` builds a `Ctx`, dispatches the message, fires due timers, and returns `ctx.effects`. The app matches on each effect, so the first variant becomes a compile error at the one place that must carry it out. Self-insert moves onto `Ctx`; `Core::next_deadline()` is public for the app.

**Bridge in the app.** `aiditor-services` depends on `aiditor-core` and exposes `channel() -> (Sender<Msg>, Receiver<Msg>)` with a fixed bound (256), the one place services and the app agree on. In `main.rs`:

```
Editor { core, tx: Sender<Msg>, timer: Option<Task<()>> }
  handle(msg, cx): effects = core.handle(Instant::now(), msg); match each effect;
                   rearm(cx); cx.notify()
  rearm(cx):       timer = core.next_deadline().map(|d| spawn: background timer(d - now).await,
                                                    then this.update(|e| e.handle(Msg::Tick)))
on open: cx.spawn(drain: while let Some(msg) = rx.recv().await { this.update(|e| e.handle(msg)) })
```

Every path into the core goes through `Editor::handle`, so re-arming cannot be forgotten. Replacing `timer` drops the old `Task`, which cancels it. `tx` stays on `Editor`: it keeps the channel open and is what effects will clone into services.

**Harness fake clock.** `Harness { now: Instant, .. }` starts at `Instant::now()`; `keys` passes `now`; `advance(d)` adds `d` and handles `Msg::Tick`.

## Risks / Trade-offs

- [A repeating timer with a zero interval makes the app wake the core every frame] → It's an internal API, not a trust boundary; a `debug_assert!` on a non-zero interval catches it in tests.
- [`Msg` must stay `Clone`] → A future variant that can't be cloned (a PTY handle, a oneshot sender) would need an `Arc`. Timer messages are small, so this is unlikely to bite.
- [The "cancelled while due" and "started while firing" scenarios have no `Msg` that cancels or starts timers yet] → They are tested at the `Timers` level by interleaving `due`, `cancel`/`start` and `fire` the way `handle` does.
- [Drain and re-arm in GPUI are untested code] → Kept to a few lines; checked by hand in the walking skeleton (#10).

## Migration Plan

The harness is the only caller of `handle`; it changes in the same commit. `docs/editor.md` ("The core runs on the UI thread") records `handle(&mut Core, now, Msg) -> Effects` and core-owned timers.
