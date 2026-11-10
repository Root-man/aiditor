# Proposal

## Why

With point in a session's prompt area the transcript is pinned to its end, and there is no mouse, so reading history means leaving the prompt and losing the draft's place. Issue #475 makes the prompt and the transcript view independent.

## What Changes

- With point in the prompt area the transcript keeps the rows it showed; it follows new output only while it shows its end, as `tail -f`. Sending a prompt shows the end again.
- `C-v`, `M-v` and `C-l` with point in the prompt area page and recenter the transcript without moving point. `C-l` recenters on the transcript point.
- The window keeps a transcript point and a prompt point. New `agent-toggle-prompt` (`C-t` in `agent-session-mode`) moves point between them, keeping the transcript's rows; the transcript point is first put within the rows shown.
- A char that would self-insert, typed with point in the transcript, inserts at the prompt point instead and moves point there. Bound keys (`TAB`, permission and question keys) keep their meaning.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: the docked prompt requirement's transcript scrolling, the read-only transcript's typing, and a new requirement for `agent-toggle-prompt`.

## Impact

- `crates/aiditor-core/src/window.rs`: the follow flag and the two remembered points.
- `crates/aiditor-core/src/frame.rs`: `session_display` keeps the transcript rows.
- `crates/aiditor-core/src/motion.rs`: scroll and recenter act on the transcript from the prompt area.
- `crates/aiditor-core/src/agents.rs`, `builtins.rs`, `lib.rs`, `keymap.toml`: the command, its binding, typing redirect and the post-command bookkeeping.
- `docs/editor.md`: the docked prompt section.
