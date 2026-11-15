## Why

Frame 14 puts pinned sessions at the top of `*Sessions*`, but nothing feeds the Pinned group yet (#482 left it empty). Issue #484 adds `P` to pin a session and keeps pins across restarts, the editor's first state of its own outside crash recovery.

## What Changes

- `P` (`agent-sessions-pin`) on a session row pins or unpins it. Pinned sessions list under Pinned, at the top, most recent first; unpinned they go back to their activity group. Point stays on the session.
- Pins are kept in `sessions.toml` in the platform's local data directory, one entry per agent, project primary root and session id, rewritten on each change and read at launch.
- When an agent's list answers for a project without a pinned session of that agent and project, the pin is dropped from the file without a message.
- The echo line hints gain `P pin/unpin`, between fold and refresh (frame 14 reads `P pin`; changed in review, as `P` toggles).
- A `sessions.toml` that can't be read or parsed is reported in `*Messages*`, and the editor starts with no pins.

## Capabilities

### New Capabilities

### Modified Capabilities
- `sessions-list`: pinning with `P`, the Pinned group, pins kept across restarts and dropped when gone, and the `P pin/unpin` key hint.

## Impact

- `crates/aiditor-core/src/sessions.rs`: pins, Pinned grouping, `P`, dropping gone pins.
- `crates/aiditor-core/src/lib.rs`: `Msg::PinsLoaded` and `Effect::WritePins`.
- `crates/aiditor-core/keymap.toml`, `builtins.rs`: `agent-sessions-pin` on `P`.
- `crates/aiditor-services/src/runner.rs`, `files.rs`: writing `sessions.toml` atomically, latest text winning.
- `crates/aiditor/src/main.rs`, `editor.rs`, the headless binary: the file's path, read at launch.
- `docs/editor.md`: Session manager and Persistence record what #484 built.
