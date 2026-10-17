# Proposal

## Why

Issue #29: the window already has a mark, but nothing sets it from the keyboard, and the display highlights the region whenever a mark exists, so once set it could never be hidden. The region commands that come next (kill, copy, send region to an agent) need Emacs's mark: `C-SPC`, `C-x C-x`, a mark ring, and `C-g` to drop the highlight.

## What Changes

- The mark gets Emacs's transient-mark-mode state: it is active or inactive, and the display highlights the region only while it is active.
- `set-mark-command` on `C-SPC`: pushes the old mark onto the window's mark ring (at most 16), sets the mark at point and activates it. Pressed again right after, it toggles the region on and off. `C-u C-SPC` jumps to the mark and rotates the ring.
- `exchange-point-and-mark` on `C-x C-x`: swaps point and mark and activates the region.
- `keyboard-quit` (`C-g`) deactivates the mark.
- Any command that changes the current buffer deactivates the mark, as in Emacs.
- `M-<` and `M->` push the mark at the old point without activating it, unless the region is active or `C-u` is given. **BREAKING** for the #27 rule that they do not set the mark.
- The frontend sends Ctrl+Space as `C-SPC`.
- Out of scope: shift-selection, `C-@`, `set-mark-command-repeat-pop`, the global mark ring, `C-u C-x C-x`.

## Capabilities

### New Capabilities
- `mark`: setting, activating and deactivating the mark, exchanging it with point, and the mark ring.

### Modified Capabilities
- `display-model`: the region is highlighted only while the mark is active.
- `keymaps`: `C-g` with no prefix pending also deactivates the mark.
- `motion`: `beginning-of-buffer` and `end-of-buffer` push the mark.

## Impact

`crates/aiditor-core/src/window.rs` (active flag, mark ring), `lib.rs` (new commands, deactivation after buffer changes, display call), `buffer.rs` (a modification tick), `keymap.toml` (`C-SPC`, `C-x C-x`), `crates/aiditor/src/main.rs` (Ctrl+Space), `docs/editor.md` (the decision).
