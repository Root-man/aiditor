# Proposal

## Why

A long build or test run in a terminal finishes silently while you look at another app or workspace. #524 laid the notification plumbing and #525 the visibility rule; this change makes long terminal commands use them (#526).

## What Changes

- A terminal command that runs, from its OSC 133 `C` mark to its `D` mark, at least `terminal_notify_after` seconds posts a notification tagged with the terminal buffer, while you can't see that buffer. The title is the command line the shell gave (`Command` without one); the body is `finished in 2m 16s`, or `failed (exit 2) after 2m 16s` for a nonzero exit code.
- A new setting `terminal_notify_after` in `settings.toml`, bundled as 10, a nonnegative integer; 0 turns it off.

Not included: shells without OSC 133 (they get none), a shell that exits mid-command, Linux (#527).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `system-notifications`: long terminal commands notify while their terminal can't be seen.
- `settings`: the `terminal_notify_after` setting.

## Impact

- `crates/aiditor-core`: `terminal.rs` times commands between marks and posts; `settings.rs` and `settings.toml` gain the setting; `agents/render.rs`'s `long_duration` becomes crate-visible.
- `docs/editor.md`: the notifications entry records what was built.
