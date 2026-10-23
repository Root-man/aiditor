# Proposal

## Why

Testing the [M1 gate (#85)](https://github.com/Root-man/aiditor/issues/85) exposed three command usability gaps: configuration reload has no completion feedback, the one-line workspace list hides buffer context, and creating a workspace needlessly requires choosing the current project again.

## What Changes

- Confirm a successful accepted configuration reload with `Configuration reloaded` in the echo area (the minibuffer's message line), preserving reload errors and existing partial-application behavior.
- Replace `workspace-list`'s echo-area summary with a temporary read-only buffer showing a project → workspace → buffers tree. Only workspace rows are selectable; Enter switches the entire workspace. Project and buffer rows are informational.
- Keep the project prompt in `workspace-new`, with the active workspace's project selected initially and other projects still available.
- Keep existing command names and global bindings. Add only local list navigation, activation and dismissal bindings.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `settings`: Confirmation after an accepted successful runtime configuration reload, without masking diagnostics or acknowledging stale results.
- `workspaces`: A temporary workspace tree with workspace-only activation, and an active-project default for workspace creation.

## Impact

- Core configuration completion and diagnostics in `configuration.rs` and `lib.rs`.
- Workspace commands and tests in `workspace.rs` and `builtins.rs`; buffer, window and minibuffer integration for the temporary list and project selection.
- Local bindings in `crates/aiditor-core/keymap.toml`, focused core tests, and the affected command descriptions in `docs/editor.md`.
- No new dependency, configuration option, persisted format or GPUI-specific widget. Existing `workspace-list` tests must change because its output intentionally moves from the echo area into a buffer.
- Out of scope: selecting projects or individual buffers from the tree, workspace management actions in the list, folding, live refresh, general tabular UI infrastructure, and completing the remaining manual M1 gate.
