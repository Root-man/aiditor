# Design

## Context

`Ctx::key` in `crates/aiditor-core/src/lib.rs` looks the pending keys up through the transient, mode and global keymaps and calls the bound command with `Args::default()`. `Core.transient` is a keymap consulted first for the next key sequence only and cleared before the command runs, so a command can set one for the following keys. `Args.prefix` is `Option<i64>`. See proposal.md for why.

## Goals / Non-Goals

**Goals:** Emacs's `universal-argument`, `digit-argument` and `negative-argument` behavior for the keys in the proposal.

**Non-Goals:** echoing the argument as it is typed (echo area, #36); `M--` and `C-<digit>` bindings; keypad digits.

## Decisions

- **Prefix keys are commands, not special cases in the loop.** `universal-argument` (`C-u`), `digit-argument` (`M-0`..`M-9`) and `negative-argument` (`C--`) are built-ins bound in the global keymap, as in Emacs, so they are rebindable and callable by name. Alternative, a state machine in `Ctx::key`: hardcodes the keys and duplicates the keymap.
- **Digits after a prefix come from the transient keymap.** Each of these commands leaves a transient map binding `0`..`9` to `digit-argument` and `C-u` to `universal-argument`, plus `-` to `negative-argument` while there are no digits yet. That is Emacs's `universal-argument-map`; the existing transient slot already gives "next key only".
- **Raw prefix in `Args`.** `Args.prefix: Option<Prefix>`, `enum Prefix { Universal(i64), Minus, Number(i64) }` with `value()`. The prefix commands need the raw form (`C-u` after `C-u` multiplies, after digits it does not), and later commands such as `C-u C-SPC` need it too. Alternative, a separate raw field in `Core`: two sources of truth.
- **Rules follow Emacs's `simple.el`.** universal: none→U(4), U(n)→U(4n), Minus→U(-4), Number kept and no transient map. digit d: Number(v)→v*10±d (sign of v), Minus→Number(-d) except d=0 keeps Minus, otherwise Number(d). negative: Number(v)→Number(-v), Minus→none, otherwise Minus. Arithmetic saturates.
- **Handlers find their digit in `this_keys`.** `Core` keeps the keys of the sequence running the command (Emacs's `this-command-keys`); `digit-argument` reads the last key's char. Alternative, ten registered commands: needs closures or ten fns.
- **The loop hands the argument over.** `Ctx::key` takes `Core.prefix_arg` into the `Args` of every key-run command and on the fallback path; a prefix command puts the new one back. When a command leaves a prefix argument, the loop restores the previous last command.

## Risks / Trade-offs

- [`C-u 3 a` types one `a` until #28] → #28 turns self-insert into a command that reads the count.
- [Plain keys after a prefix argument ignore it, so `C-u 3 -` types one `-`] → the fallback clears the argument; #28 adds the count.
