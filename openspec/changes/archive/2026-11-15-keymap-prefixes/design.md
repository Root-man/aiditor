# Design

## Context

`keymap::parse` binds every file's tables in order into `HashMap<String, Keymap>`; a `Binding` is a command or an anonymous `Prefix(Keymap)` made on the way by `Keymap::bind`. The toml tables come back sorted by name, so `[global]` is read before `[project-prefix]`.

## Goals / Non-Goals

**Goals:** named prefixes from any file, bound under any number of sequences in any keymap; unbinding with `false`; the hint naming them.

**Non-Goals:** Emacs's live sharing of a prefix map after load (the keymaps are rebuilt only on load, so a copy is the same thing); defining prefix commands in Rust; shadowing a lower layer's key (`false` only removes from its own keymap).

## Decisions

- **Resolve after loading.** Binding a sequence to a table name stores an empty `Prefix` keymap carrying that name. Once every file is bound, each named prefix is filled with a copy of its table (itself resolved), and anything bound under the sequence directly (`"C-x p z" = ...` in `[global]`) is laid over the copy. Binding at load order would miss tables read later, and the sort order puts `global` first. Alternative, a `Binding::Named` resolved at lookup: every lookup, hint and `where_is` would need the map table.
- **A value names a command first, then a table.** `swiper` is both a command and a minibuffer keymap, and binding the command must keep working. A name that is neither is reported as `no command or keymap named <name>`.
- **Cycles are reported.** A prefix whose table binds itself, directly or through others, is reported as `<name>: prefix cycle` and that inner binding is left an empty prefix.
- **`false` removes the binding** at that sequence in its keymap (`Keymap::unbind`); nothing bound there is not an error. `true` is reported as not a command name.
- **The prefix tables stay keymaps by name**, as every table is; nothing names them as a mode, so they are only reached through their sequences.
- **Hint** shows `+<name>` for a named prefix and `+prefix` for an anonymous one.

## Risks / Trade-offs

- A user's `[project-prefix]` entry changes the prefix under every sequence, but a sequence-specific `"C-x p z"` changes only that copy → documented.
- `"C-x p f" = false` does nothing, as unbinding runs before the prefix is filled; a key in a prefix is unbound in its table (`[project-prefix] f = false`) → shortcut comment on `unbind` and docs/editor.md.
