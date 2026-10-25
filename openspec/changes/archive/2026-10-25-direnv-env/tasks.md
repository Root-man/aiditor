# Tasks

## 1. Services

- [x] 1.1 Add `environment::direnv(vars, dir)`: find `direnv` on the captured `PATH`, run `direnv export json` with the captured environment and `TERM=dumb`, apply the JSON, 30 s timeout; verify with fake `direnv` scripts: applied set and unset, empty output, a non-zero exit with its stderr, bad JSON, no `direnv` on `PATH`

## 2. Core and app

- [x] 2.1 Add `direnv: Option<String>` to `Msg::EnvironmentCaptured` and log it in `*Messages*`; update the senders (app task, tests); verify with a harness test that a direnv failure keeps the vars and notes the project and reason
- [x] 2.2 Run the direnv step after a successful capture in the app's capture task

## 3. Docs and checks

- [x] 3.1 Update `docs/editor.md` (Process environment, direnv) and run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
