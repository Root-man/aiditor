# Proposal

## Why

A whole-repo refactor scan found 19 opportunities: bugs that can kill the editor, hang an agent session or silently rewrite a file, slowness on large grep lines and command output, and structure that makes planned session work harder. They are small and independent enough to land together, and most of them shorten the code.

## What Changes

Numbers are the scan's rows.

Bugs and risks:

- 1: Stop reading device files and pipes as files, and leave files over 10 MiB out of previews, so browsing `/dev` can't exhaust memory or stall previews.
- 2: Keep reading an agent's stderr past non-UTF-8 bytes, so the agent can't block on a full pipe.
- 3: Treat a file as CRLF only when every line break is CRLF, so mixed files save byte for byte. **BREAKING** for mixed files that start with CRLF: they now show their `\r`s and stay mixed on save.
- 6: Flush new-file and hard-link saves to disk before reporting them, as other saves do.
- 9: Refuse to replace a file the user has no write permission for. **BREAKING**: saving a `0444` file in a writable directory now fails instead of succeeding through rename.
- 10: Ask before `workspace-kill` ends live terminals or agent sessions, as quitting and `kill-buffer` do. **BREAKING** for scripts of keys that kill such a workspace: they meet one more question.
- 11: Don't run `git status` when the repository's own config defines a content filter, which `vcs-branch` already forbids.

Performance:

- 4: Cut project grep's listed rows to the visible width, as swiper does.
- 5: Defer command-output rendering in hidden sessions, and append new output to folded calls instead of rebuilding the block each frame.
- 15: Set the window title only when it changes.
- 19: Replace linear hit lookups and per-batch path cloning in project grep.

Lean and structure, with no change in behavior:

- 7: Delete `Session.updates`, which nothing reads, and handle each update in one `match`.
- 8: Move the session's pure text rendering out of `agents.rs`.
- 12: Group `Core`'s loose auto-save and watch fields into structs.
- 13: Replace command-name checks in generic prompt code with `ArgSpec` fields.
- 14: Split `Editor::render` by job.
- 16: Share the config-option flattening in `agents.rs`.
- 17: Keep the ignore-file name rule in one place.
- 18: Pass `configure_walker` a path instead of a one-element `Vec`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-visiting`: Non-regular files are refused, mixed line endings round-trip, saves refuse files the user can't write, and reported saves are flushed.
- `minibuffer`: File previews leave out non-regular files and files over the size limit.
- `agent-processes`: Stderr with non-UTF-8 bytes keeps streaming.
- `project-grep`: Listed rows hold no more of a line than can be shown.
- `session-buffers`: Command output in hidden sessions is rendered when the session is next shown.
- `agent-sessions`: Raw updates are no longer kept; the rule dropping updates for unknown or ended sessions stays.
- `workspaces`: Killing a workspace asks about its live processes.
- `vcs-branch`: A repository's own content filters are never run.

## Impact

- `aiditor-services`: `files.rs`, `agents.rs`.
- `aiditor-core`: `buffer.rs`, `agents.rs` and a new `agents/render.rs`, `grep.rs`, `minibuffer.rs`, `command.rs`, `builtins.rs`, `lib.rs`, `autosave.rs`, `watching.rs`, `visiting.rs`, `workspace.rs`, `file_index.rs`, `project.rs`, and the hot-path benches.
- `aiditor`: `editor.rs`.
- `docs/editor.md` records the new behavior. No dependencies, settings or public file formats change.
- Panics outside commands stay out of scope; #168 tracks them.
