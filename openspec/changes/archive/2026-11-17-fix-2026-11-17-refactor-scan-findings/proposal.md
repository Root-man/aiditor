# Proposal

## Why

The 2026-11-17 refactor scan of the whole repo found 20 problems. Two are user-visible freezes or panics in the session buffer: sending a big paste freezes the UI, and a stale scroll row panics redisplay on every frame. The rest are smaller bugs, risks, slow paths and design problems in code that M8 and M9 change next. Fixing them together, while the findings are fresh, is cheaper than finding them again one by one.

## What Changes

Each finding keeps its row number from the scan.

**Bugs and risks**
- **#1 Big pastes freeze on send:** `user_lines` counts the text's chars once per line. Count them once.
- **#2 Stale `top_row` panics redisplay:** a session window whose top line has fewer wrapped rows than its stored `top_row` indexes out of bounds in `window_rows` and `Window::shown`. Clamp it where it is read.
- **#5 `C-c C-k` answers the request at point:** like `C-c C-y`, it answers the request whose block point is on, and otherwise the oldest pending request as today.
- **#6 `revert-buffer` defaults to No:** its prompt lists `No` first, so `RET` keeps the buffer, as every other destructive prompt does.
- **#7 `find-file` refuses huge files:** a regular file over 256 MiB is not read, and the echo area names the file and its size, instead of the editor running out of memory.
- **#11 Resize re-lays out the whole frame:** the modeline and minibuffer take the new width on the frame after a resize, not on the next message.
- **#12 A save of unchanged text does not conflict:** a save whose bytes are already on disk counts as written, so saving one buffer twice before the first reply no longer reports `changed on disk` or cancels a quit.
- **#18 Grep column never panics:** a match that starts inside a multi-byte char no longer slices the line there.
- **#19 `aiditor --wait +LINE` moves point in a window already showing the file.**
- **#20 Rolling back keeps the pin:** a pinned session's pin moves to the fork that replaces it.

**Performance**
- **#3 Prompt repaint without a transcript walk:** skip removing properties from the prompt area when it holds none.
- **#4 Session search cut in the worker:** the grep service reduces session JSONL lines to what was said before sending them to the core.
- **#8 Grep labels without the line text:** a project grep hit's label is its path and line, so the matched line is held once.
- **#9 Project file prompt builds its candidates once.**
- **#14 Wrapped windows lay out only the rows they show.**
- **#16 Grammars without a locals query record no edited nodes.**
- **#17 `*Messages*` keeps its last 1,000 lines.**

**Design**
- **#10 One language table:** file extensions and Markdown fence names come from one table, so every extension a file is highlighted by also works as a fence name.
- **#13 `agents.rs` split by job:** its tests move to `agents/tests.rs`; asks, tool calls and config options move to `agents/asks.rs`, `agents/tools.rs` and `agents/config.rs`. Moves only.
- **#15 Grep line numbers faced in the core:** the core gives the minibuffer the line number's face range, and the frontend stops re-parsing the label.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: `C-c C-k` answers the request at point first (#5); rolling back moves a pin to the fork (#20).
- `agent-questions`: `C-c C-k` skips the question at point first (#5).
- `file-reverting`: `revert-buffer`'s question defaults to `No` (#6).
- `file-visiting`: files over 256 MiB are refused (#7); a save whose bytes are already on disk succeeds (#12).
- `syntax-highlighting`: fence names cover every language's extensions and full name (#10).
- `messages-buffer`: `*Messages*` keeps its last 1,000 lines (#17).

The other findings keep behavior as the specs already describe it (#2, #11 and #19 are bugs against existing requirements).

## Impact

- Core (`aiditor-core`): `agents.rs` and `agents/` (render, blocks, new `tests.rs`, `asks.rs`, `tools.rs`, `config.rs`), `sessions.rs`, `frame.rs`, `window.rs`, `display.rs`, `buffer.rs`, `syntax.rs`, `syntax/locals.rs`, `grep.rs`, `minibuffer.rs`, `project_commands.rs`, `visiting.rs`, `edits.rs`, `lib.rs`.
- Services (`aiditor-services`): `files.rs` (size limit, unchanged-bytes save), `grep.rs` (column, session-line reduction).
- Frontend (`aiditor`): `element.rs` (resize frame, grep line-number faces).
- Benchmarks: a `hot_paths` case with wrapped paragraphs (#14) and one for a long-session prompt keystroke (#3).
- Docs: `docs/editor.md` for the decided limits (256 MiB, 1,000 lines), `docs/refactoring-opportunities.md` for the scan record.
- No new dependencies, settings or data formats. Not in scope: the ~18 smaller scan findings left out of the table.
