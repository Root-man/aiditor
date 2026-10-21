# Tasks

## 1. The finder

- [x] 1.1 `project-find-file` in `project_commands.rs`/`builtins.rs`: root names (`<2>` for repeats), candidates from the current workspace project's index, its own `Find file in <project>: ` question, the answer mapped to a path and visited with `find_file`; index on first use. Bind `C-x p f`. Verify with harness tests for two roots, repeated root names, a typed new file under a root and under the primary, and no preview.
- [x] 1.2 `indexed` pushes the walk's files into an open finder for that project. Verify with a harness test: first use echoes `Indexing`, the list fills when the walk answers, and a second open starts no walk.
- [x] 1.3 Update the `C-x p` prefix-hint test for ten commands.

## 2. Docs

- [x] 2.1 Record the finder, index on first use and the no-preview decision in `docs/editor.md`.

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
