# Proposal

## Why

`crates/aiditor-core/src/lib.rs` is 2366 lines and holds most of the editor: one `impl Ctx<'_>` block of about 60 methods (`lib.rs:235-1382`), a `Core` struct of about 40 ungrouped fields (`1384-1452`), the whole command table inside `impl Default for Core` (`1459-1799`) and `impl Core` (`1801-2330`). `harness.rs` is 2867 lines, nearly all of it one `mod tests`. Every M1 feature (projects, workspaces, grep, auto-revert) would land in the same file, so split it now, before M1 starts and while no other branch is editing `aiditor-core`.

The command table is also fragile: handlers sit in a fixed-size `[(&str, Handler); 57]` array and their specs are patched in afterwards by name with 7 `expect("a builtin")`, so a renamed command panics at startup and a duplicate name silently replaces the first.

## What Changes

- `impl Ctx<'_>` and `impl Core` move, mostly verbatim, into child modules of the crate root, one per subsystem, named after the `openspec/specs/` capabilities where one fits. `lib.rs` keeps the message types, `Ctx`'s command loop (`call`, `try_call`, `dispatch`, `key`), the `Core` struct and `handle()`.
- The tests in `harness.rs` move next to the modules they test; `harness.rs` keeps the `Harness` and the helpers several modules share. Same test names, same count.
- Two state structs come out of `Core`: `KillRing` (the kill ring, yank pointer and clipboard fields, with `kill_new`, `current_kill` and `take_clipboard` as its methods) and `Prompts` (the open minibuffer, histories, matcher and repoll timer, with `matches` and `selected_candidate`). The command-loop and auto-save fields stay on `Core`.
- The command table becomes one `builtins() -> Vec<Command>` with each spec inline; building the map asserts names are unique. No array length, no `expect("a builtin")`.
- `dispatch`'s `Saved`, `Listed` and `AutoSave` arms become named methods (`on_saved`, `on_listed`, `write_autosaves`); `recover_buffers` calls `write_autosaves` instead of dispatching `Msg::AutoSave`.
- No behavior changes; the public API of `aiditor-core` used by `aiditor` and `aiditor-services` is unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: this is a pure refactor and every behavior stays as `openspec/specs/` specifies. The change sets `skip_specs: true`.

## Impact

- `crates/aiditor-core/src/`: `lib.rs` shrinks to a few hundred lines; new modules `builtins.rs`, `editing.rs`, `motion.rs`, `kill_ring.rs`, `mark.rs`, `prefix.rs`, `minibuffer.rs`, `frame.rs`, `visiting.rs`, `autosave.rs`, `bindings.rs`; `harness.rs` loses its `mod tests`.
- No other crate changes, no new dependencies; `aiditor-core` still has no gpui or tokio dependency.
- A large, mostly-moved diff: it conflicts with any branch in flight on `aiditor-core`, so it lands after `m0-quick-wins` and `clip-long-lines` and before `shared-effect-executor` and `prompt-choices`.
