# Proposal

## Why

#52 contains panics only inside command handlers. A panic while the frontend hands the core a service result, fires a timer, handles a key outside a command, or builds the frame still ends the process; on macOS, inside the key callback, it aborts.

## What Changes

- Every message the frontend hands the core (keys, typed text, service results, timer wake-ups, resizes) is handled under panic containment: the editor keeps running, the echo line says the core panicked and its state may be inconsistent, and `*Messages*` gets the panic and a backtrace, as for commands.
- Building the frame for redisplay is contained the same way; a frame that cannot be built is drawn with no windows and the echo line, so the user can still save and quit.
- The command wrapper and the new ones share one containment helper in the core.

## Capabilities

### New Capabilities

### Modified Capabilities

- `core-loop`: panics while handling any message or building the frame are contained.

## Impact

- `crates/aiditor-core/src/lib.rs`: a public containment method shared with `Ctx::try_call`.
- `crates/aiditor/src/editor.rs`, `crates/aiditor/src/element.rs`: `Editor::handle` and the frame builds go through it.
- `docs/editor.md`: the "Contain panics" note loses its "Not yet".
