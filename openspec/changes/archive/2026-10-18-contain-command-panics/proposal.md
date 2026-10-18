# Proposal

## Why

The editor is experimental and a bug in one command must not take it down with unsaved edits (issue #52, `docs/editor.md` Crash safety). Today a panic in any command handler unwinds out of the core and ends the process.

## What Changes

- Each command handler runs under `catch_unwind`. A panic is caught, the echo line says the command panicked and its state may be inconsistent, and `*Messages*` gets the panic message, its location and a backtrace. The editor keeps running.
- Any build with `panic = "abort"` fails to compile, so CI (and every local build) catches a profile that would silently disable the containment.
- Not in this change: the frontend wrapper around non-command calls into the core (dispatch of service answers, timers, rendering). `docs/editor.md` records it as not yet built.

## Capabilities

### New Capabilities

### Modified Capabilities
- `commands`: a panicking command is contained and reported instead of ending the process.

## Impact

- `crates/aiditor-core/src/lib.rs`: `Ctx::try_call` wraps the handler; a chained panic hook captures the backtrace; a `compile_error!` under `cfg(panic = "abort")`.
- `crates/aiditor-core/src/harness.rs`: a test.
- `.github/workflows/ci.yml`: a step that fails on `panic = "abort"` in any manifest.
- `docs/editor.md`: Crash safety notes what was built.
