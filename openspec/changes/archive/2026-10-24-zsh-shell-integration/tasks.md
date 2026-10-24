# Tasks

## 1. Zsh integration

- [x] 1.1 Write `crates/aiditor-services/src/zshenv`: ZDOTDIR restore, user `.zshenv`, deferred precmd/preexec hooks; check by hand under a PTY with starship
- [x] 1.2 Give `Terminals` an optional integration folder; on a default-program zsh spawn write `.zshenv`, set `ZDOTDIR`/`AIDITOR_ZDOTDIR`; `shortcut:` comment for other shells; verify `cargo test -p aiditor-services`
- [x] 1.3 Pass `<data dir>/shell/zsh` from `main.rs` through `Editor` to `Terminals`; verify `cargo build`
- [x] 1.4 Integration tests in `crates/aiditor-services/tests/terminal.rs` for the spec's scenarios (skip when zsh is missing); verify they pass
- [x] 1.5 Record the decision in `docs/editor.md`'s Terminal paragraph
- [x] 1.6 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass

## Workflow follow-up

- Archive the change after review.
