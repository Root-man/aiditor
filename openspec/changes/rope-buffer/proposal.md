# Proposal

## Why

The core keeps one hardcoded rope and point in `Core` (marked `shortcut:` in `crates/aiditor-core/src/lib.rs`). Markers, undo, the window model, the walking skeleton (#10) and everything after them need real buffers: content addressed by id, an edit API, line/column positions and a file binding. This is issue [#14](https://github.com/Root-man/aiditor/issues/14), the head of M0's longest dependency chain.

## What Changes

- Buffer ids and a buffer registry in the core: create, look up and kill buffers by id.
- A buffer holds a name, its content, an optional file path and a modified flag.
- Insert and delete by char offset. Out-of-range edits are refused with an error, never a panic, because the core must not panic on input (see `docs/spikes/gpui.md`).
- Conversion between char offsets and (line, column), with lines broken only at LF.
- Content is held behind a single-variant enum (`Text`), so the terminal grid can become a second variant later without changing callers. The session buffer is a rope (decision #8), so it uses `Text` too.
- `Core` keeps a registry with a scratch buffer instead of its own rope; self-insert goes through the buffer edit API. The harness test keeps passing.

Out of scope: loading or saving files (#10, then the file-io task), buffer kinds, unique buffer names, UTF-16 offsets for the input handler (#10), markers (#15) and undo (#16).

## Capabilities

### New Capabilities

- `text-buffers`: buffers in the core: ids and registry, char-offset editing, line/column positions, the modified flag and the file binding.

### Modified Capabilities

None.

## Impact

- `crates/aiditor-core`: new `buffer` module; `Core` switches to the registry.
- `crates/aiditor-core/Cargo.toml`: ropey's default features turned off (keeping `simd`), so only LF breaks lines.
- No change to the frontend or services crates.
