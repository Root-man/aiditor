# Proposal

## Why

The 2026-11-11 architectural review found five structural problems in the core that the M7 rich session and M8 session rework would build on. Two cost time on the UI thread today: the perf scan of 2026-10-25 measured about 21 ms per keystroke typing `/*` in a 10k-line Rust file, 186–445 ms for the delimiter pair at 100k lines, and 33–141 ms for the first parse of a 10k-line file. The other three make the session and buffer code harder to change than it needs to be: the session keeps seven parallel collections with their own markers, a buffer's kind is spread over flat fields and side maps that are polled after every message, and the effect runner is written four times.

## What Changes

Each item keeps its number from the review.

- **#1 Session transcript as one ordered block list:** a session keeps its transcript as one ordered list of blocks. Each block has one start and one end marker, and one function writes a changed block into the buffer and repairs its markers. Finding the block at point is one search of that list. The seven collections (`messages`, `tools`, `plan`, `asks`, `answers`, `run`, `groups`) stop holding markers of their own.
- **#2 Parsing off the UI thread:** a buffer larger than 64 KiB is parsed by a service. The core draws with the old tree, shifted by the edits, until the new one arrives. Smaller buffers still parse in the core, so they are coloured in the frame that draws the edit, as today.
- **#3 Buffer kind payload:** `Buffer` gets a `kind` holding what only that kind needs (terminal, session, workspace list, sessions list), and the file-only fields move into one file state. Every buffer kill goes through one function, which emits the terminal and agent effects. `sync_terminals`, `sync_agents` and `sync_session_lists` stop polling for dead buffers, and the 16 `agents.values().find(|s| s.buffer == …)` scans become a lookup.
- **#4 One effect runner:** `aiditor-services` gets a runner that carries out an `Effect` and hands back only what the caller must schedule or do itself. The app, `tests/scenario.rs` and `tests/file_sync.rs` use it. `aiditor-headless` keeps its small synchronous loop.
- **#5 Per-message tail:** measure what the 13 passes at the end of `Core::handle` cost per message with many buffers. If the watch set's rebuild costs more than 20 µs per message, build it only when its inputs changed. If not, record the number and drop the item.
- **Measurements and records:** baseline and final benchmark runs in `docs/measurements/`, `docs/performance.md`, the review recorded in `docs/refactoring-opportunities.md`, and `docs/editor.md` updated where it describes parsing and buffer kinds.

Only #2 changes behavior: in a buffer larger than 64 KiB, syntax colours can trail an edit by the time one parse takes. Items #1, #3, #4 and #5 keep the observable text, faces, folds, motion, commands, effects and messages as they are.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `syntax-highlighting`: "Highlighting follows the text" allows colours to trail the text in a buffer larger than 64 KiB, and new requirements say what is drawn meanwhile, that a large buffer's parse never runs in the core, and that a late result is handled.

## Impact

- Core (`aiditor-core`): `lib.rs` (`Core`, `Msg`, `Effect`, `handle`), `buffer.rs`, `syntax.rs`, `frame.rs`, `visiting.rs`, `minibuffer.rs`, `workspace.rs`, `terminal.rs`, `sessions.rs`, `watching.rs`, `autosave.rs`, `workspace_list.rs`, `agents.rs`, `agents/groups.rs`, `agents/questions.rs`, `agents/render.rs`, `agents/pastes.rs`, and nearby tests.
- Services (`aiditor-services`): a new runner module, `tests/scenario.rs`, `tests/file_sync.rs`. `bin/aiditor-headless.rs` gains the parse effect's arm only.
- Frontend (`aiditor`): `editor.rs` (`Editor::handle` and its service fields).
- Public API: `Effect` and `Msg` each gain one parse variant. `Buffer`'s public file fields move behind a file state. No new dependencies, settings, commands or data formats.
- Benchmarks: new cases for the per-message tail, typing in the prompt of a long session, and an edit frame in a large buffer without its parse. Existing case names, fixtures and sampling stay unchanged; the `syntax-edit-frame` and `revert-apply-frame` cases at 10k and 100k lines stop including the parse, which the report must say.
- Not in scope: a faster marker or property store (the review's claim that #1 would retire the `buffer.rs` marker shortcut does not hold, since properties, not block bookkeeping, hold most markers; the new benchmark records the cost for a later decision), frame 7's rich session, re-wrapping a transcript when the window's width changes, moving the terminal emulator or the core off the UI thread, and replacing `aiditor-headless`'s loop.
