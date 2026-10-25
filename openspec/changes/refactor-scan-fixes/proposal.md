# Proposal

## Why

Two refactoring scans of `d3c9565` ([refactoring-opportunities.md](../../../docs/refactoring-opportunities.md), [perf-scan-2026-10-25.md](../../../docs/perf-scan-2026-10-25.md)) found concrete bugs (a recovery panic, leaked PTYs, a recovery file deleted after a failed write) and hot paths that cost whole frames (long lines, coloured copy mode, per-frame highlighting, a 50 ms first grep batch). Each was checked against the code; the ten that hold up are fixed here together because they are small and mostly independent.

## What Changes

Bugs:
- **A1** A terminal whose shell exits, or whose spawn fails, releases its PTY, input thread and service handle at once instead of at app quit.
- **A2** Recovering a buffer whose visited file has since become read-only (binary) replaces its text instead of panicking.
- **A6** An auto-save that replaces a recovered file deletes the old recovery file only after the new one is safely written.

Performance:
- **A3** Drawing a window over a very long line skips the hidden columns directly and highlights only the columns shown.
- **A4** Building copy-mode text keeps a running char count for soft-wrapped lines instead of recounting the growing line each row.
- **A7** `Buffer::char_idx` reads a line's last char directly instead of walking the line.
- **B1** A grep sends its first non-empty batch at once instead of after the 50 ms deadline.
- **B2** Drawing coloured copy mode looks up only the properties near the window instead of every property in the scrollback.
- **B4** Syntax highlights are reused across frames while text, range and palette are unchanged.

Benchmarks:
- **B5** The 100k-line Rust/TSX syntax fixtures nest their items, so the tree root no longer has 100k children.

Out of scope:
- **A5** parallel project grep: deferred to M5 in [#265](https://github.com/Root-man/aiditor/issues/265).
- **B3** reparsing off the core thread: an architectural change for the M5 syntax epic's roadmap, not this refactor.
- Retrying a failed regular auto-save (the doc's ack/retry design): a failed write keeps the previous recovery file, so no data is lost; only the recovery-replacement path deletes anything.

## Capabilities

### New Capabilities

None.

### Modified Capabilities
- `terminal-buffers`: after the shell exits or its spawn fails, the core tells the terminal service to release it (today it sends nothing).
- `terminal-processes`: killing a terminal whose shell already exited or never started sends no signal and releases its PTY and input thread.
- `auto-save`: recovering into a now read-only file keeps the recovered text; the old recovery file is deleted only after the current session's copy is written.
- `project-grep`: the first non-empty batch is sent as soon as it has hits.

## Impact

- `aiditor-core`: `terminal.rs` (`end_terminal`, `copy_text`), `autosave.rs`, `buffer.rs` (`char_idx`, `add_property`, properties near a range), `display.rs` (`build`), `syntax.rs` (highlight ranges and cache), `frame.rs`, `lib.rs` (`RecoveryOp::AutoSave`), core benchmark scenarios.
- `aiditor-services`: `grep.rs` (first batch, deadline test), `recovery.rs` (delete after rename).
- `docs/performance.md`: note the changed syntax fixtures and grep first-batch meaning, since earlier results are not comparable.
- No new dependencies, settings or user-visible commands.
