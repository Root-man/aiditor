# Design

## Context

`Ctx::key` runs a keymap binding when there is one; otherwise, for a single plain key, it inserts the char, inserts a newline for `RET` or deletes back for `DEL` by hand. The motion commands (#27) are entries in `Core::default`'s `builtins` table, bound in `keymap.toml`, and take their count from `Args::count()`.

## Goals / Non-Goals

**Goals:** every editing key is a named command in `builtins`, takes the prefix argument and can be rebound.

**Non-Goals:** the kill ring (its own issue); indentation for `C-j`; amalgamating consecutive deletes into one undo group (only typed chars amalgamate, as now); `Msg::Insert` (#12).

## Decisions

- **Self-insert stays a fallback.** A TOML keymap can't bind every printable char, and #21 decided that an unbound plain key still types. So `Ctx::key` calls `self-insert-command` for an unbound plain `Char` key through `Ctx::call`, with the prefix and `this_command` set like a bound command. The command reads its char from `this_keys`, as `digit-argument` does. Any other unbound key is undefined. A user binding for a plain char still wins, because the fallback runs only after lookup fails.
- **One insert helper and one delete helper.** `insert_n(text, n)` inserts `text` repeated `n` times at point. A negative `n` errors. It moves point after the text, or leaves point where it was for `open-line`. `delete_n(n)` deletes the `n` chars after point, or `-n` before it. When that runs past an end it deletes nothing and returns Emacs's end error. `delete-backward-char` is `delete_n(-n)`.
- **Kill-word reuses the word motion.** `forward_word` is split so that a `word_end(n) -> usize` gives the target position without moving. `kill-word` deletes `point..target`, and `forward_word` goes to the target. A backward count gives a target before point, so the range is ordered first.
- **`C-j` is its own name.** It is bound to `electric-newline-and-maybe-indent` with the same handler as `newline`, so the indentation issue only swaps the handler and leaves the binding alone.
- **Point after an insert.** The window's point marker is `Stay`, so commands set point after inserting, as the fallback did.

## Risks / Trade-offs

- [`M-DEL` relies on the frontend reporting alt+backspace as `Key { meta, Backspace }`] → it does, since `to_keys` maps named keys with their modifiers. Covered by a key-script test in the core only.
- [Kill commands lose the text] → shortcut comment pointing at the kill-ring issue.
