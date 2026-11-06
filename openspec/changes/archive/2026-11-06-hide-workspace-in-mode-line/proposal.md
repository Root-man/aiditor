# Proposal

## Why

Buffers scoped to a workspace carry its name as a suffix (`vterm1<Aiditor>`, `*scratch*<Aiditor>`, `*claude stderr*<Aiditor>`, `*claude output: x*<Aiditor>`), and the buffer mode line shows that name verbatim. The workspace modeline already shows `<project> › <workspace>`, so the suffix repeats it, against frame 9's rule that nothing the workspace bar shows repeats in a buffer line. File buffers show no workspace, so the two kinds of buffer disagree in the same window.

## What Changes

- The mode line of a terminal or other fileless buffer shows its name without the current workspace's `<workspace>` suffix: `vterm1`, `-- *scratch*`, `%% *claude stderr*`.
- Buffer names themselves are unchanged. `switch-to-buffer`, `find()`, persistence and uniqueness still see `vterm1<Aiditor>`.
- Buffers owned by no workspace (`*Messages*`, `*Workspaces*`) and names without the suffix show as before.
- Agent session mode lines (which show the session title, not the buffer name) and file mode lines (which show the path) are untouched.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: "Each window has a mode line" shows a terminal's or fileless buffer's name without its workspace suffix.

## Impact

- `crates/aiditor-core/src/frame.rs`: `window_display` gets the current workspace's name and trims it from a fileless buffer's mode line name; its callers pass it, or `None` from `minibuffer.rs`.
- Tests and snapshots whose mode line text includes a `<workspace>` suffix.
