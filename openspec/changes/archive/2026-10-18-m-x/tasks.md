# Tasks

## 1. Core

- [x] 1.1 Add `ArgSpec::annotation`, store `(candidate, annotation)` in the matcher, and add `annotations` to `display::Minibuffer`; verify with a harness test that annotations are listed and not matched, and update snapshots
- [x] 1.2 Add `Core::where_is` and the `execute-extended-command` builtin with its spec, bind `M-x` in the global keymap; verify with harness tests for running by name with a prefix, narrowing, unknown names and annotations (`C-x 2`, none for `exit-minibuffer`)

## 2. Frontend and docs

- [x] 2.1 Draw each candidate's annotation after it in `crates/aiditor/src/main.rs`; verify the frontend completion test still passes and `cargo clippy` is clean
- [x] 2.2 Record the decisions in `docs/editor.md` (Commands and Minibuffer bullets); verify the text names `M-x`, annotations and the where-is rule

## Workflow follow-up

- Archive the change after merge preparation.
