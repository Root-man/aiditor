# Proposal

## Why

Typing, `RET` and `DEL` are hardcoded fallbacks in the key handler (#22), so they ignore the prefix argument, can't be rebound and `M-x` can't run them, and `C-j`, `C-o`, `C-d`, `M-d` and `M-DEL` don't exist. Issue #28 turns them into named commands that honour the prefix argument, as the motion commands did in #27.

## What Changes

- New commands, under Emacs's names: `self-insert-command`, `newline`, `electric-newline-and-maybe-indent`, `open-line`, `delete-char`, `delete-backward-char`, `kill-word` and `backward-kill-word`. Each repeats by the prefix argument's value, so `C-u 3 a` types `aaa`.
- The bundled global keymap binds `RET` to `newline`, `C-j` to `electric-newline-and-maybe-indent`, `C-o` to `open-line`, `C-d` to `delete-char`, `DEL` to `delete-backward-char`, `M-d` to `kill-word` and `M-DEL` to `backward-kill-word`.
- An unbound plain character key runs `self-insert-command` instead of hardcoded insertion. The hardcoded `RET` and `DEL` handling goes away.
- A delete that would pass an end of the buffer deletes nothing and reports `End of buffer` or `Beginning of buffer`, as Emacs does. A `DEL` at the start of the buffer still leaves it unchanged, but now shows that message.
- `kill-word` and `backward-kill-word` delete the text but don't save it yet, because the kill ring is its own issue.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `editing-keys`: typing, `RET` and `DEL` become named commands that take the prefix argument. Requirements are added for `C-j`, `C-o`, `C-d`, `M-d` and `M-DEL`, and for deleting past a buffer end.

## Impact

- `crates/aiditor-core/src/lib.rs`: the new commands and the narrower fallback in `Ctx::key`.
- `crates/aiditor-core/keymap.toml`: the new default bindings.
- `crates/aiditor-core/src/harness.rs`: key-script tests.
- `crates/aiditor/src/main.rs`: a test that the editing keys, `M-DEL` included, reach the core as the key scripts name them. No frontend code changes, because keys already reach the core as `Msg::Key`.
