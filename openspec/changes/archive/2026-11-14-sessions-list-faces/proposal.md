## Why

#482 gave `*Sessions*` frame 14's groups and columns, but as plain text: no colours, bracketed states, and every column padded from the left so the agent and time drift with the longest title. Issue #483 draws it as frame 14.

## What Changes

- Faces per frame 14: the heading's marker in `annotation`, its name in a new `session-list-group` (Pinned in `session-list-pinned`, Archived in `annotation` bold) and its count in a new `session-list-count`; the title in the default face; the agent as `✻ claude` or `◆ codex` in new `agent-claude` and `agent-codex` faces; times in `mode-line-dim`, Today's in a new `session-list-today`.
- **BREAKING** (text): states read as the workspace list's words with a dot: `● working`, `● awaiting permission`, `● awaiting answer`, `● exited`, `○ idle`, and, settled here, `○ loading` and `○ starting` in the idle face. They replace `[working]`, `[waiting]`, `[ended]`, `[idle]`, `[loading]` and `[starting]`.
- **BREAKING** (text): rows start four columns in, under the heading's name; the agent, time and state columns sit at the window's right edge (an `AlignRight` space after the title), so titles are no longer padded. Headings read `▾ <name>  <count>`.
- Point lands on a row's title and a heading's name, as frame 14's cursor; point's row gets the selected-row background the workspace list uses.
- The mode line's `<m> open` shows in `status-running` when any session is open.

## Capabilities

### New Capabilities

### Modified Capabilities
- `sessions-list`: row layout and state words, heading layout, faces, point's column and row background, the mode line's open count.

## Impact

- `crates/aiditor-core/src/sessions.rs`: rendering with faces, `AlignRight`, point's column, row background, mode line.
- `crates/aiditor-core/src/agents.rs`: `session_state` returns the state's words and face.
- `crates/aiditor-core/src/settings.rs`, `settings.toml`: six new faces.
- `crates/aiditor-core/src/frame.rs`: unchanged call, `decorate` now also sets the row background.
- `docs/editor.md`: the faces table and the rework paragraph.
