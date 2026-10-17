# Tasks

## 1. Commands

- [x] 1.1 Add `insert_n` and `delete_n` helpers and split `word_end` out of `forward_word` in `crates/aiditor-core/src/lib.rs`; verify the existing motion tests still pass
- [x] 1.2 Register `self-insert-command`, `newline`, `electric-newline-and-maybe-indent`, `open-line`, `delete-char`, `delete-backward-char`, `kill-word` and `backward-kill-word` in `builtins`; verify with `cargo test -p aiditor-core`
- [x] 1.3 Make `Ctx::key` call `self-insert-command` for an unbound plain char and drop the hardcoded `RET`/`DEL` handling; verify the existing typing, undo and undefined-key tests pass
- [x] 1.4 Bind `RET`, `C-j`, `C-o`, `C-d`, `DEL`, `M-d` and `M-DEL` in `crates/aiditor-core/keymap.toml`; verify the bundled keymap still parses (the `Core::default` assert)

## 2. Tests and docs

- [x] 2.1 Add key-script tests in `crates/aiditor-core/src/harness.rs` for every scenario in the `editing-keys` delta, including `C-u 3 a`; verify `cargo test --workspace` passes
- [x] 2.2 Record the decisions in `docs/editor.md` under Commands and keymaps; verify the paragraph is there
- [x] 2.3 Update the `editing-keys` main spec's Purpose, since typing is no longer only unbound keys

## Workflow follow-up

- Archive the change after verify.
