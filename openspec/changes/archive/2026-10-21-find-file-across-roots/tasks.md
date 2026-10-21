# Tasks

## 1. The finder

- [x] 1.1 `project-find-file` in `project_commands.rs`/`builtins.rs`: root names (`<2>` for repeats), candidates from the current workspace project's index, its own `Find file in <project>: ` question, the answer mapped to a path and visited with `find_file`; index on first use. Bind `C-x p f`. Verify with harness tests for two roots, repeated root names, and a typed new file under a root and under the primary.
- [x] 1.2 `indexed` pushes the walk's files into an open finder for that project. Verify with a harness test: first use echoes `Indexing`, the list fills when the walk answers, and a second open starts no walk.
- [x] 1.3 Update the `C-x p` prefix-hint test for ten commands.

## 2. Preview

- [x] 2.1 `Preview::{Buffer, File}` hook; `FileOp::Preview`/`Msg::Previewed` in core, services and harness; `Core::preview` reads unvisited files into one hidden read-only preview buffer, drops stale answers, kills it on the next read and on close; a command with no declared prompts previews its own question. `C-x b` gives buffers, `find-file` and `project-find-file` give files. Verify with harness tests: finder preview, `C-x C-f` file and directory, late read dropped, `C-g` leaves no buffer, `RET` visits normally; a services test for `FileOp::Preview`.

## 3. Docs

- [x] 3.1 Record the finder, index on first use and file preview in `docs/editor.md` (and the roadmap entries that dropped preview).

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
