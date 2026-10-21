# Design

## Context

`Core` holds a single window configuration in four fields: `window` (the selected window), `selected` (its id), `others` (every other window, the minibuffer's included while a prompt is open) and `tree` (the split tree). Every window command, the redisplay and the input paths use those fields directly. Window ids come from a global counter, `next_window`. A window's point and mark are markers in its buffer.

## Goals / Non-Goals

**Goals:** a saved configuration per inactive workspace; a core call that switches workspaces; window and frame code left unchanged.

**Non-Goals:** switch, next/previous and the other commands and keys (#64); the workspace strip (#65); scoping `C-x b` (#66); keeping a prompt open across a switch.

## Decisions

- **The current configuration stays in `Core`'s fields; inactive ones are saved on their `Workspace`.** `Workspace` gets `windows: Option<Windows>`, where `Windows` holds the same four values. It is `None` while the workspace is current. A switch swaps `Core`'s four fields with the target's saved `Windows` and stores the old ones on the old workspace. The alternative was to move the four fields into one `Windows` struct and index it by workspace. That touches every window command and the frame code for no behavior difference.
- **Window ids stay global.** `next_window` keeps counting across workspaces, so no two windows ever share an id. A click or scroll the frontend sends for a window of a workspace that is no longer current fails the existing `tree.leaves()` check and is dropped, as a deleted window's is.
- **Inactive windows keep their markers.** Their buffers stay live, so points and marks follow edits made while away (reverts, the other workspace's edits through a shared buffer until #66), at no extra cost.
- **A switch closes the minibuffer first** with `close_minibuffer`, as a click does. The minibuffer's window lives in `others`, so it would otherwise travel into the saved configuration.
- **A switch raises the restored selected window's buffer**, so the most-recent order starts with the buffer on screen, as `switch_to` keeps it.
- **A new workspace builds a fresh single-window configuration** on its scratch: one new window id and a `Tree::Leaf`. It then installs that configuration as a switch does.
- **Killing the current workspace switches to the neighbour first.** Then the killed workspace's saved windows are released, which removes their markers, and its buffers are killed. When `kill_buffer` runs, no window of the killed workspace is left.
- **`kill_buffer` walks every configuration.** For the current one it keeps today's rule: the most recent listed buffer of the current workspace or `*Messages*`, else a new `*scratch*`. For each saved configuration the same rule is applied with that workspace's id. Every live workspace still owns its unkillable `*scratch*`, so a replacement always exists there. If none were found, the window would fall back to the current workspace's replacement rather than panic.

## Risks / Trade-offs

- [A prompt open during a switch is cancelled] → #64's commands run after their prompt closes, so this only matters for a programmatic switch. A shortcut comment marks it.
- [The frame size is shared] → a restored configuration is laid out again at the next redisplay, as after a resize. No per-workspace size is kept.
