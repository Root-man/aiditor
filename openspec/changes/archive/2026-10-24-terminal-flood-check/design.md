## Context

See proposal.md for why. `Editor::drain` spawns one GPUI foreground task per channel (the shared service channel and one per terminal). Each PTY reader thread does `blocking_send` into a 4-deep channel; parsing a 64 KB batch takes about 0.6 ms in the core (new benchmark), so the reader is always ahead during `yes`.

## Goals / Non-Goals

**Goals:** measure the flood's cost per batch and in the running app; make sure the UI thread still handles keys and draws during one.

**Non-Goals:** moving the emulator into the terminal task (only a follow-up, if measurements show lag); coalescing batches or a per-frame time budget for output.

## Decisions

- **Yield after every message.** After handling a message, the drain task awaits `tokio::task::yield_now`, which outside a Tokio runtime wakes the task at once and returns Pending, re-queuing it behind pending UI work. Zed's terminal does the same with `futures_lite::future::yield_now`; aiditor doesn't depend on `futures-lite`, and Tokio is already a dependency. A zero-length GPUI timer doesn't work: `timer(Duration::ZERO)` returns a ready task. A yield costs one executor round trip per message, small next to a 0.6 ms batch. Alternative rejected: drain up to N messages or a time budget before yielding — more code, and nothing yet shows one yield per message is too slow.
- **Every channel, not just terminals.** The shared channel uses the same `drain`; yielding there is harmless and keeps one code path.
- **Benchmarks in the core suite.** `terminal-output/64k-batch-of-yes`, `terminal-output/64k-batch-of-cat` feed one batch through `Core::handle`; `terminal-display/50x120-screen` builds a full screen's runs. They land in the existing table and CI workflow without script changes.

## Risks / Trade-offs

- [A re-queued task may run before input on some platform] → the GPUI test proves other foreground work interleaves; the manual flood check on macOS and Linux confirms keys and frames.
- [Throughput drops slightly with a yield per message] → flow control already throttles the producer; output keeps up with the screen.
