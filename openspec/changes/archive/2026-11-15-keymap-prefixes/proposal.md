# Proposal

## Why

A prefix such as `C-x p` exists only as the first keys of each binding, so moving the project commands to another key means rebinding every command, and the old keys stay bound because `keymap.toml` cannot unbind (#185).

## What Changes

- A key in `keymap.toml` can be bound to the name of another table, a named prefix keymap: `"C-x p" = "project-prefix"` with `[project-prefix]` holding `c = "project-create"` and so on. One entry moves or adds the whole prefix; a prefix can sit on several sequences.
- An entry whose value is `false` unbinds that sequence in its keymap.
- The bundled keymap moves the project commands to `[project-prefix]` on `C-x p`, the workspace commands to `[workspace-prefix]` on `C-x t`, and adds `[agent-prefix]` on `C-x a` (`a` agent-start, `l` agent-sessions, `f` agent-fork-session, `r` agent-rollback-session, `m` agent-select-model, `e` agent-select-effort).
- The which-key hint shows a named prefix as `+<name>` instead of `+prefix`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `keymaps`: bindings may name a prefix keymap or be `false`; the bundled project, workspace and agent bindings live in named prefixes; the hint names named prefixes.

## Impact

`crates/aiditor-core/src/keymap.rs` (parse, `Keymap`), `bindings.rs` (hint), `keymap.toml`, the `C-x` hint snapshot, `docs/editor.md` (Commands and keymaps).
