# Tasks

## 1. Raw prefix in Args

- [x] 1.1 Add `command::Prefix` (`Universal`, `Minus`, `Number`, `value()`), change `Args.prefix` to `Option<Prefix>` and update the existing callers and tests; verify with `cargo test -p aiditor-core` and a test that a `C-u` prefix of 4 differs from the number 4 with equal values

## 2. Prefix argument commands

- [x] 2.1 Add `prefix_arg` and `this_keys` to `Core`; make `Ctx::key` pass the prefix argument to the command it runs, clear it on any other path, and keep the last command when a command leaves a prefix argument; verify with harness tests for only-next-command, `C-g` and last command
- [x] 2.2 Add `universal-argument`, `digit-argument` and `negative-argument` with Emacs's rules and the transient map, bound to `C-u`, `M-0`..`M-9` and `C--`; verify with harness tests for `C-u 3` (3), `C-u C-u` (16), `C--` (-1), `C-u 1 2`, `C-u - 5`, `M-4 2` and `C-- 7`

## 3. Docs and checks

- [x] 3.1 Record the decision in `docs/editor.md`'s Prefix argument bullet
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, all passing
