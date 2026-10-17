# Proposal

## Why

The text model (rope edits, markers, grouped undo/redo) has only example-based tests plus one marker property test. Issue #17 asks for property tests over random edit sequences so undo/redo round trips and marker bounds are checked on inputs nobody thought to write by hand.

## What Changes

- A `proptest` property in `aiditor-core`'s buffer tests: random inserts and deletes (including non-ASCII text) with random undo boundaries, on a buffer holding markers of both insertion types.
- It asserts that undoing everything returns the original text, redoing everything returns the final text, and every marker stays within `0..=len_chars` after each edit, undo and redo.
- CI already runs `cargo test` on Linux and macOS, so the property runs there with no workflow change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Tests only; no requirement changes (`skip_specs: true`).

## Impact

- `crates/aiditor-core/src/buffer.rs` (tests module only). `proptest` is already a dev-dependency.
- Any bug the property finds is fixed in `buffer.rs` in this change.
