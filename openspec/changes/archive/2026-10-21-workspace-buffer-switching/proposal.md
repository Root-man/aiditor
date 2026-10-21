# Proposal

## Why

Every buffer belongs to one workspace, but `C-x b` still completes over every workspace's buffers, so another task's files leak into this one's switcher and can be shown in its windows (#66). The spec (docs/editor.md, Workspaces) says the switcher lists the current workspace's buffers only.

## What Changes

- `switch-to-buffer` (`C-x b`), `switch-to-buffer-other-window` (`C-x 4 b`) and `kill-buffer` (`C-x k`) complete over the current workspace's buffers plus the global `*Messages*` only.
- The name they read is looked up in that same set: a name owned only by another workspace counts as unknown. `C-x b` and `C-x 4 b` then make an empty buffer here (uniquified, e.g. `a.txt<2>`, since names stay unique across workspaces); `C-x k` fails with `No such buffer <name>`.
- No consult-style switcher exists yet; `C-x b`'s completion is the switcher, so nothing else changes.
- docs/editor.md records it as built and drops the "Not yet: `C-x b` lists every workspace's buffers" note.

## Capabilities

### New Capabilities

### Modified Capabilities
- `buffer-switching`: switching and killing by name complete over and find only the current workspace's buffers and `*Messages*`.

## Impact

- `crates/aiditor-core/src/visiting.rs`: `buffer_names`, `switch_to_name`, `kill_buffer`.
- `crates/aiditor-core/src/buffer.rs`: a lookup by name scoped to the current workspace.
- docs/editor.md, Workspaces section.
