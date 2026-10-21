# Proposal

## Why

Consult-style commands show the selected candidate live in the window the prompt was opened from, so the user sees where a choice leads before committing to it, and `C-g` takes them back as if nothing happened (#69). Buffer switching wants it now; the file finder (#70) and project grep will use the same hook.

## What Changes

- A command's spec can name a preview hook that maps the selected candidate to a buffer.
- While such a command's minibuffer is open, the window it was opened from shows the selected candidate's buffer, following every change of the selection (moving it, typing, matches arriving).
- Closing the minibuffer in any way (`C-g`, `RET`, a click, a workspace switch) first puts that window back exactly as it was: buffer, point, mark, mark ring and scroll. `RET` then runs the command as before.
- Previewing a buffer does not make it the most recent one.
- `switch-to-buffer` (`C-x b`) previews; `C-x 4 b`, `kill-buffer`, `M-x` and `find-file` do not.

## Capabilities

### New Capabilities

### Modified Capabilities
- `minibuffer`: a completing prompt can preview its selected candidate in the window it was opened from, restored on close.
- `buffer-switching`: `switch-to-buffer` previews the selected buffer.

## Impact

- `aiditor-core`: `command::ArgSpec` (new `preview` field), `minibuffer.rs` (preview and restore), `lib.rs` (preview after each message), `builtins.rs` (`switch-to-buffer`'s hook).
- No frontend change: the frame display already draws every window.
- `docs/editor.md`: the Minibuffer bullet records what was built.
