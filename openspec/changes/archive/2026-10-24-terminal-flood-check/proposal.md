## Why

The terminal emulator parses on the UI thread ([#226](https://github.com/Root-man/aiditor/issues/226), docs/editor.md#output-floods), so a sustained flood such as `yes` could make typing lag. The app drains each channel with `while let Some(msg) = rx.recv().await`. Outside a Tokio runtime, Tokio's `recv` never returns Pending while a message is queued, and the PTY reader refills a terminal's channel faster than the core parses it. A flood can then keep the drain task running forever, so no key or frame is handled. We need numbers, and a drain that cannot starve the UI.

## What Changes

- Core benchmarks for one 64 KB terminal output batch (`yes` and `cat` output) and for building a 50x120 terminal screen's display, run by `scripts/bench_local.py` and the benchmarks workflow.
- The app's channel drain yields to the UI executor after each message, so input and frames run between output batches.
- Measured typing and frame times during a `yes` and a `cat` flood, on Linux and macOS, recorded in docs/editor.md under Output floods; a roadmap follow-up to move the emulator off the UI thread only if typing still lags.

## Capabilities

### New Capabilities

### Modified Capabilities
- `core-loop`: draining a channel lets input and rendering run between its messages.

## Impact

`crates/aiditor-core/benches/core.rs`, `crates/aiditor/src/editor.rs` (`Editor::drain`), `docs/editor.md`. No new dependencies.
