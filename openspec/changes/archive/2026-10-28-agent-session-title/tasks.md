# Tasks

## 1. Core

- [x] 1.1 Add `session_name(agent, workspace, title)` and a `workspace` field to `Session`, and have `agent_start` use them. Verify that the existing `agent_start` tests still see `*claude*<…>` (cargo test -p aiditor-core agents).
- [x] 1.2 Handle `SessionUpdate::SessionInfoUpdate` in `agent_update`: a value renames through `Buffers::rename`; null or an empty title renames back to the base name; undefined does nothing. Verify with task 1.3.
- [x] 1.3 Add core unit tests covering every scenario in the delta spec: the agent titles the session, a later title renames again, a long or multi-line title, a `_meta`-only update, clearing, two sessions with one title, and a workspace rename. Each test checks the name of the buffer the selected window shows (cargo test -p aiditor-core agents).

## 2. Docs

- [x] 2.1 In `docs/editor.md`, in the session buffer paragraph (the "Built in #292" bullet under Agents), record that `session_info_update` names the buffer `*<agent>: <title>*<<workspace>>`. Include the first-line and 40-char cut, that null clears the title, that updates without a title are ignored, that the buffer renames on every title, that workspace renames leave it alone, and that a future `rename-buffer` must refuse session buffers. Verify by reading the paragraph back.
- [x] 2.2 In `docs/editor.md`'s session manager bullet, note that the title is the one from `session_info_update` that the session buffer shows. Verify by reading the bullet back.

## 3. Check

- [x] 3.1 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets` and confirm both pass.
