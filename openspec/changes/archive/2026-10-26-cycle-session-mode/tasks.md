# Tasks

## 1. Core

- [x] 1.1 Keep the latest config options in the session from `config_option_update` and set answers, and remember the mode value last sent; the refusal note names it (existing default-mode tests pass)
- [x] 1.2 Add `agent-cycle-mode` (builtins, `S-TAB` in `agent-session-mode` in keymap.toml) with the readiness and no-modes echoes; core tests cover cycling through every mode and back, following an agent update, no modes, not ready, ended, and a refused cycle

## 2. End to end and docs

- [x] 2.1 Services test: `S-TAB` against the fake agent moves the mode line `[plan]` then `[auto]`
- [x] 2.2 Record the key and behaviour in docs/editor.md (session buffer section and Modes), and verify with `cargo fmt`, `clippy -D warnings` and `cargo test --workspace`
