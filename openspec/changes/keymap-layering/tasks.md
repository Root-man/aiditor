# Tasks

## 1. Keymap trie

- [x] 1.1 Derive `Hash` on `Key`/`KeyCode` and add a `kbd`-form `Display`; verify with a round-trip unit test against `parse_keys`
- [x] 1.2 Add `keymap.rs` with `Keymap`, `Binding`, `bind` and `lookup`; verify with a unit test for nested prefixes and rebinding

## 2. Layered lookup in the core

- [x] 2.1 Add `major_mode` and `minor_modes` to `Buffer`, and `keymaps`, `pending` and `transient` to `Core`; build the global map with the existing bindings plus `C-g` → `keyboard-quit`, and remove `prefix_c_x`
- [x] 2.2 Resolve keys in `Ctx::key` through the layers, with undefined messages, `C-g` cancel and the plain-key editing fallback; verify existing harness tests still pass
- [x] 2.3 Add harness tests for every scenario in `specs/keymaps/spec.md` (minor shadows global and removal restores it, mode prefix vs global, major vs minor, transient once, `C-x 4 f`, undefined messages, `C-g`)
- [x] 2.4 Record in `docs/editor.md` (Commands and keymaps) the decisions: full-sequence layer lookup, modes by name, plain unbound keys type
