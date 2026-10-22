# Proposal

## Why

Grep, the file finder, swiper and `C-x b` all jump, but nothing takes you back: after following a grep hit the only way home is to find the file and line again. Issue [#76](https://github.com/Root-man/aiditor/issues/76) asks for a position recorded before every jump and commands to go back and forward across buffers. It follows Emacs 29's xref history (`xref-go-back`, `xref-go-forward`), which jump to definition can share once it exists.

## What Changes

- Each workspace keeps a jump history: a back list of at most 100 positions and a forward list. All its windows share it, and other workspaces have their own.
- Jumps record the position they leave (buffer and point):
  - showing another buffer in the selected window, through `C-x b`, `C-x 4 b`, `find-file`, `project-find-file` and a grep hit in another file;
  - a grep hit in the current buffer;
  - a swiper jump that moves point.
- Every jump clears the forward list. A jump from the newest back position adds nothing to the back list.
- `M-,` (`xref-go-back`) goes to the newest back position, in the selected window, and puts the position it left on the forward list. `C-M-,` (`xref-go-forward`) does the reverse. Both skip positions in killed buffers and positions where point already is. When nothing is left they say `At start of xref history` or `At end of xref history`.
- Positions are buffer markers, so they follow edits.
- Previews record nothing, and neither do going back and forward.
- Not included: a prefix argument to clear the history (`C-u M-,`), a history per window (`xref-history-storage`), recording `M-<`, `M->` or clicks, and listing the history.

## Capabilities

### New Capabilities
- `jump-history`: what is recorded as a jump, the per-workspace back and forward lists, and `xref-go-back` and `xref-go-forward`.

### Modified Capabilities

## Impact

- `aiditor-core`:
  - a new `jump.rs` with the history and its two commands;
  - the `Workspace` struct;
  - `Core::switch_to` in `visiting.rs`;
  - `project_grep` in `grep.rs`;
  - `swiper` in `swiper.rs`;
  - workspace kill, which releases the history's markers;
  - command registration in `builtins.rs`;
  - `keymap.toml`.
- `docs/editor.md`: the Search and navigation paragraph. Project grep's "Not yet" list drops jump history.
