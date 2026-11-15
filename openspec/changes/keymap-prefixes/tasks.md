# Tasks

## 1. Keymap parsing

- [x] 1.1 Give `Keymap` an optional prefix name; bind a table name as a named prefix, `false` as unbind, and resolve named prefixes after all files (overlay, cycle report); unit tests in `keymap.rs` for two sequences, overlay, unbind, unknown and cyclic names
- [x] 1.2 Hint shows `+<name>` for named prefixes; `C-x` hint snapshot updated

## 2. Bundled keymap and behavior

- [x] 2.1 Move project and workspace bindings into `[project-prefix]`/`[workspace-prefix]`, add `[agent-prefix]` on `C-x a`; existing project/workspace key tests still pass
- [x] 2.2 Harness key-script test: user file binds `C-c p` to `project-prefix` and unbinds `C-x p`; every project command is under `C-c p` and `C-x p f` is undefined; plus a test that both sequences work when `C-x p` is kept

## 3. Docs

- [x] 3.1 `docs/editor.md` Commands and keymaps records named prefixes, `false`, the agent prefix and the hint name; `cargo fmt`, clippy and tests pass
