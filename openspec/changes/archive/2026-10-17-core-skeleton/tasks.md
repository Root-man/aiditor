# Tasks

## 1. Timers in the core

- [x] 1.1 Add `crates/aiditor-core/src/timer.rs` (exported as `pub mod timer`) with `TimerId`, `Timers` (`start`, `cancel`, `due`, `fire`, `next_deadline`) and `Timer`, as in design.md, with a `debug_assert!` on a non-zero repeat interval. Derive `Clone` and `Debug` on `Msg`. Add unit tests that interleave `due`, `cancel`/`start` and `fire` the way `handle` does: cancelled while due returns `None`, a zero-delay timer started while firing is not in the snapshot, a late repeat fires once and its next deadline is `now + interval`, and `next_deadline` is the earliest deadline or `None`. Verify with `cargo test -p aiditor-core timer`.

## 2. Timestamped handle, effects and Ctx

- [x] 2.1 In `crates/aiditor-core/src/lib.rs`, add `Effect` (empty), `Effects`, `Ctx` with `start_timer`/`cancel_timer`, `Msg::Tick`, a `timers` field on `Core` and `Core::next_deadline()`. Change `handle` to `handle(&mut self, now: Instant, msg: Msg) -> Effects`: dispatch the message through `Ctx`, then fire due timers from a `due` snapshot. Move self-insert onto `Ctx`. Verify with `cargo build -p aiditor-core`.
- [x] 2.2 In `harness.rs`, add the fake clock (`now` field, `keys` passes it, `advance(Duration)` handles `Msg::Tick`) and a way for tests to start and cancel timers through a `Ctx`. Add harness tests for the timer spec's one-shot, repeating, late wake-up, cancel-before-deadline and next-deadline scenarios, using `Msg::Key` payloads and checking the buffer text. Verify with `cargo test -p aiditor-core`, including the existing typing test.
- [x] 2.3 In `docs/editor.md` ("The core runs on the UI thread"), change the signature to `handle(&mut Core, now, Msg) -> Effects` and add one bullet: timers live in the core as deadlines, fire as messages, and the app wakes the core at `next_deadline()`. Verify by reading the section back.

## 3. Service bridge in the app

- [x] 3.1 Make `aiditor-services` depend on `aiditor-core` and add `channel()` returning a bounded (256) `tokio::sync::mpsc` pair of `Msg`. Add a `#[tokio::test]` where a test service spawned on the runtime sends `Msg::Key` for `x`, and the received message handled by a `Core` makes its text `x`. Verify with `cargo test -p aiditor-services`.
- [x] 3.2 In `crates/aiditor/src/main.rs`, give `Editor` `tx` and `timer: Option<Task<()>>`, route every message through `Editor::handle` (core `handle` with `Instant::now()`, match each effect, re-arm, `cx.notify()`), and spawn the drain task when the window opens. Verify with `cargo build -p aiditor` and by running `cargo run -p aiditor` to check that the window still opens and closes cleanly.

## 4. Integration

- [x] 4.1 Run CI's checks, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the no-GPUI-or-Tokio check (`cargo tree -p aiditor-core | grep -E '\b(gpui|tokio) v'` finds nothing), and verify all pass.

## Workflow follow-up

- Open a PR that closes #19, then archive the change after merge.
