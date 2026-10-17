# Proposal

## Why

Keys are matched by hardcoded `if`s and a `C-x` flag, so there are no prefix keys beyond `C-x`, no way for a mode to rebind a key, and undefined keys fail silently. The command loop, TOML keymaps and which-key all need a real keymap to build on (issue #21).

## What Changes

- A keymap is a trie of key sequences: each key maps to a command name or to a nested prefix keymap, so `C-x`, `C-c` and `C-x 4` are ordinary prefixes.
- A key sequence is looked up in layers, highest first: the transient map, the current buffer's minor-mode maps, its major-mode map, then the global map. The first layer that binds the sequence (as a command or a prefix) wins.
- Every buffer has a major mode (default `fundamental-mode`) and a list of minor modes; their keymaps are held by the core by mode name.
- The existing bindings (`C-x C-s`, `C-x C-r`, `C-/`, `C-_`, `C-x u`, `C-?`) move into the global map; the hardcoded `C-x` flag goes away.
- An undefined key sequence with Ctrl or Meta, or after a prefix, shows `<keys> is undefined` on the message line. Plain unbound keys still type and move as today.
- `C-g` cancels a pending prefix and shows `Quit`; on its own it runs the new `keyboard-quit` command, which also shows `Quit`.

Not in this change: commands to turn modes on and off, keymaps from TOML, the which-key hint, the command loop.

## Capabilities

### New Capabilities
- `keymaps`: key sequences, prefix keys, layered keymap lookup, undefined keys and quitting a prefix.

### Modified Capabilities
None. `C-x C-s`, `C-x C-r` and the undo keys keep their specified behavior.

## Impact

`aiditor-core`: new `keymap` module, `Key` gains `Hash` and a `kbd`-style `Display`, `Buffer` gains mode fields, `Ctx::key` resolves keys through the keymaps. The frontend is unchanged.
