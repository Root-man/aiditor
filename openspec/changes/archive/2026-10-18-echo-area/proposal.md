# Proposal

## Why

The core already shows messages and errors on a message line, and the frontend draws it as the echo area, but the frame display model leaves it out. Display snapshots can't show what the user sees at the bottom of the frame, so issue #36 (Echo area) asks that command errors appear in both.

## What Changes

- The frame display carries the echo area's text: the current message, or none.
- The echo area's lifetime is written down as a contract: a message stays until the next key or click, and `C-g` shows `Quit` (already specified under keymaps).
- Nothing changes in what the frontend draws.

## Capabilities

### New Capabilities
- `echo-area`: what the echo area shows and when a message is cleared.

### Modified Capabilities
- `display-model`: the frame display includes the echo area message.

## Impact

- `crates/aiditor-core/src/display.rs` (`FrameDisplay`), `crates/aiditor-core/src/lib.rs` (`Core::frame`).
- Frame snapshots in `crates/aiditor-core/src/snapshots/` gain the message field.
- No frontend, API or dependency changes beyond the new field.
