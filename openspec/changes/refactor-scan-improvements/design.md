# Design

## Context

See `proposal.md` for the 19 rows and `specs/` for the behavior. Three things shape the approach:

- The core is a plain state machine and services do the IO, so file and process fixes stay in `aiditor-services` and the core only sees their results.
- `files::read` is the one read path for open, revert, diff, check and launch. `FileOp::Preview` has its own `fs::read`.
- `show_tool` rebuilds a call's whole text and diffs it against the block. #404 deferred hidden streamed messages but kept "existing dirty tool-call handling", and no bench covers tool output.

## Goals / Non-Goals

**Goals:**

- Fix each row in the shared code, once, with the smallest change that works.
- Keep the lean rows free of behavior change: existing tests pass with test edits limited to moved or renamed names.

**Non-Goals:**

- Panics outside commands (#168), and capping `*Messages*`, undo history or transcript memory.
- A large-file prompt or lazy loading: `RET` on a big regular file still reads it whole.
- Fsync of parent directories: after a crash a replaced file still holds its old content, and a new file's name may need the kernel's next journal commit.
- A settings entry for the preview limit.
- Incremental rendering of open tool calls, which keeps full re-rendering with a `shortcut:` comment.

## Decisions

### Rows 1 and 3: reading and decoding

`files::read` gets `fs::metadata` first and returns an error for anything that is neither a regular file nor a directory. Directories fall through to `fs::read`, so their existing OS error and the test that asserts it stay. Metadata never blocks on a FIFO, which is why the check comes before any open.

Preview gets a shared helper applying the same check, plus `PREVIEW_LIMIT = 10 MiB`. Non-regular files return `Err` and oversized ones return `Ok(None)`, both of which `on_previewed` already treats as "window as it was". Answering is what clears `active_preview`, so a pipe no longer blocks later previews. 10 MiB is well past any file worth previewing and keeps a selection move under one fast read.

`decode` sets `crlf` only when the text has a line break and every `\n` follows `\r`. Mixed files then take the existing LF path, which already keeps `\r` and round-trips. The first-break rule was rejected because it can't round-trip a mixed file.

### Rows 6 and 9: saves

One `write_synced(path, bytes)` helper (create or truncate, write, `sync_all`) replaces both `fs::write` calls in `files::write`. `sync_all` on macOS already uses `F_FULLFSYNC`.

Before choosing how to write, `files::write` opens the canonical target for writing without truncating, and returns only `PermissionDenied`. Other errors such as `ETXTBSY` continue to the rename path, as today. Checking mode bits was rejected: it misses a file owned by someone else in a writable directory, and the open answers exactly what the OS would allow. Tests that need a denied write skip under root.

### Row 11: repository filters

`git status` has no flag that disables filters, and `--attr-source` still reads `.git/info/attributes`. So `changed` first runs `git config --show-scope --get-regexp '^filter\.'`, which runs no filter. A line whose scope isn't `global` or `system` means the file counts as unchanged. Exit 1 means no matches and continues to `status`; any other failure counts as unchanged, as "no usable git" does. `--show-scope` covers `local`, `worktree` and includes in one call. It needs git 2.26 or later, and older git falls into the failure branch. This adds a second process per file to an existing `shortcut:`, which keeps its upgrade path.

### Row 2: stderr

`read_until(b'\n')` into a reused `Vec<u8>`, then strip `\n` and an optional `\r`, then `String::from_utf8_lossy`. That matches `lines()` on valid text. The loop ends only at EOF, a read error or a closed channel. The fake agent gets a script step that writes raw bytes, so the test exercises the real pipe.

### Rows 8, 7 and 16: session code

These go first because row 5 edits the same functions. Row 8 moves `render`, `rewrite`, the header, row, ask and content builders, `coloured`, `fenced` and `render_tool` to `agents/render.rs`, following the `syntax.rs` and `syntax/locals.rs` layout. Only visibility changes.

Row 7 deletes `updates` and its `shortcut:`, and turns the six `match &update.update` blocks into one `match update.update` by value. That also removes the clones of tool calls. Row 16 adds `options(select)`, which `values` and `mode_line` both use.

### Row 5: command output

The ANSI state moves onto `Term`: a named `Sgr` handler and a `Processor` kept across chunks. `terminal_meta` feeds each chunk once, so colouring costs only the new bytes and an escape split across chunks still parses. `tool_content` reads the kept text and faces.

`render_streams` checks `shown` for dirty tools as it does for streams. A dirty call that is folded, already shows output and only gained output takes a short path:

1. It takes the kept text without its one trailing newline, the part `fenced` drops.
2. It inserts the new tail just before the closing fence with `replace_output`, which extends the `Invisible` property covering the block.
3. It updates `len` and `faces`.

Everything else takes the full `show_tool` path: the first output, an exit change, a new backtick run as long as the fence, an open call, or a call whose fields changed. Open calls are left as they are because `render_tool` would need incremental code-block properties, while calls start folded. That limit gets a `shortcut:` comment.

### Rows 4 and 19: project grep

Row 4 cuts `hit.text` by chars to `columns` at `minibuffer.rs:775`, leaving `Hit` whole for preview and jump. `Hit.text` itself stays full because `col` and `RET` need it.

Row 19 makes three changes:

- `ProjectGrep` keeps a persistent file-to-group map and stores each hit's group number, so sorting clones no paths.
- `line_number_width` becomes a running maximum.
- `minibuffer()` and `grep::preview` read hits by item index, because `f.items` and `hits` share an order. A new `Prompts::selected_index` gives the preview that index.

`hit(label)` stays for the accept handler, which runs once. `set_items` still clones labels per batch, which gets a `shortcut:` comment.

### Row 10: workspace kill

`save_buffers_kill_terminal`'s `PROCESSES` handling moves to one `Ctx` helper taking the command, args, whether anything is live, and the prompt. `kill_workspace` calls it after `save_some`, with "live" meaning a terminal or unended session whose buffer belongs to that workspace. The direct `Core::kill_workspace` stays unconditional, because `on_saved` and tests reach it after the questions.

### Rows 12 to 15 and 17, 18: structure

- **12:** `autosave::Autosave` holds the auto-save fields and `recovered`, and `watching::Watch` holds `watching`, `watch_refresh` and `watch_refusals`. This is the `Prompts` and `ProjectGrep` pattern. Fields keep their names without the prefix.
- **13:** `ArgSpec` gains `unselected` for find-file and project-find-file, `project_candidates` for workspace-new, and `grep_hits` for project-grep. Swiper is already identified by `Filter::Swiper`. No trait or new layer is added.
- **14:** `echo_area`, `hint_rows` and `workspace_bar` become `Editor` helpers. Nothing changes in element ids, so the `debug_selector` tests stay the check.
- **15:** `Editor` keeps the last title and skips the call when it matches.
- **17:** `project::is_ignore_file` is public in core and the watcher imports it.
- **18:** `configure_walker` takes `&Path`.

## Risks / Trade-offs

- [Mixed files show `\r`] → Accepted: it is the honest view and already happens for files whose first break is LF.
- [Oversized previews look like missing files] → `RET` still visits them. A message would be noise while moving through a list.
- [Repositories with their own filters lose the changed marker] → Spec-required by "no command from its config". Git LFS installs its filter globally, so it keeps working.
- [The output short path can drift from a full rebuild] → A test compares folded and open calls over the same chunks, including a backtick run that forces the full path.
- [Row 5 builds on row 8's move] → Tasks order them, and each group passes its tests before the next starts.
- [Row 13's flags are command-shaped] → Accepted: three booleans on an existing spec beat string checks, and a trait would be a layer the problem doesn't need.
- [One broad change] → Task groups are independent enough to land as separate commits.
