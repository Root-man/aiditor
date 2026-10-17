# Design

## Context

`Ctx::key` matches undo, redo and `C-x` keys with `if`s and a `prefix_c_x` flag, then falls through to hardcoded editing keys. Commands are registered by name (`Core.commands`), so bindings only need names.

## Goals / Non-Goals

**Goals:** a keymap trie, layered lookup, a pending sequence, undefined-key messages, `C-g` quitting.

**Non-Goals:** mode toggle commands, TOML keymaps, which-key, the command loop and this/last command, prefix arguments. Editing keys stay hardcoded as the unbound-key fallback until the editing-command tasks bind them.

## Decisions

- **Trie as nested maps.** `Keymap { HashMap<Key, Binding> }` with `Binding::Command(&'static str) | Binding::Prefix(Keymap)`. `lookup(&[Key]) -> Option<&Binding>` walks it. Binding a sequence creates prefixes on the way and replaces whatever was there. Simpler than a flat map of sequences, and which-key can list a prefix's children directly.
- **Layer lookup by full sequence.** Each key is pushed onto `Core.pending: Vec<Key>`; each layer is asked for the whole sequence and the first non-`None` wins, as Emacs does. So a mode's `C-x a` doesn't hide global `C-x C-s`. Alternative, descending into the first prefix found, would make a mode's prefix shadow the global one.
- **Modes by name.** `Core.keymaps: HashMap<&'static str, Keymap>` holds `global` and each mode's map; `Buffer.major_mode: &'static str` (default `fundamental-mode`) and `Buffer.minor_modes: Vec<&'static str>` (first has priority) name them. Mode maps are shared between buffers, as in Emacs, and a missing map is skipped.
- **Transient map.** `Core.transient: Option<Keymap>`, consulted first and taken once a sequence ends (command run or undefined), like Emacs's `set-transient-map`.
- **Linux shift.** Linux reports `C-?` and `C-_` with shift held; the global map binds both forms (`C-S-?`, `C-S-_`) instead of normalizing keys in code.
- **C-g.** Checked before lookup when a prefix is pending; on its own it is bound in the global map to `keyboard-quit`.
- **Messages.** `Key` gets a `Display` in `kbd` form (`C-x`, `M-S-a`, `RET`, `<left>`), the inverse of `parse_keys`, used for `<keys> is undefined`.
- **Undo grouping.** The self-insert check uses "no pending sequence" in place of `!prefix_c_x`.

## Risks / Trade-offs

- [Unbound plain keys always type, so a mode cannot make a plain key undefined] → fine until self-insert becomes a bound command in the editing tasks.
