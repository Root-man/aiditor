# Design

## Context

- `buffer.rs` already holds the file format helpers the file service calls: `decode` (bytes to LF text, CRLF and BOM flags, or a refusal reason), `encode` and `hash`. The service (`files::run`) runs each `FileOp` on Tokio's blocking pool and returns one `Msg`. The harness answers the same ops in-process, since the core can't depend on the services crate.
- `Buffer::insert` and `Buffer::delete` record into the open undo group and move every marker. `undo_boundary(point)` closes a group. Window points, marks, jump history and property ends are all markers.
- `set_saved` clears `modified`. Auto-save drops the recovery file of a buffer that is no longer modified after every message.
- A Yes/No question is a re-entrant command: `kill_buffer` calls `read_string` with the command itself and the choices, and runs again with the answer as the next string argument.

## Goals / Non-Goals

**Goals:**
- one pure diff function, shared by the service and the harness;
- one apply path that later callers (auto-revert, "Take disk") reuse.

**Non-Goals:**
- char-level diffs;
- a time limit on the diff;
- retrying a stale revert.

## Decisions

- **`similar` for the diff.** It is already in `Cargo.lock` through `insta`, so it adds no new crate. `imara-diff` is faster on huge files but would be a new dependency, and it works on interned tokens, which means more glue code.
- **Line granularity.** The diff runs `TextDiff::from_lines` over the buffer's text and the decoded file text.
  - Each non-equal op becomes `(Range<usize>, String)`: the old char range and its new text. Char offsets are summed from the token slices themselves, so `similar`'s line splitting never has to agree with ropey's.
  - Lines outside a hunk are untouched, so the markers on them keep their line and column.
  - shortcut: a marker inside a changed hunk goes to the hunk's start; diff the hunk by chars if point jumping to the start of an edited line gets annoying.
- **`buffer::revert(old: &Rope, read) -> Result<Revert, String>`** is pure. It takes the read result (`Result<Option<(bytes, hash)>, String>`, as `FileOp::Open` already produces) and decodes and diffs it. The two error messages, a missing file (`File <path> no longer exists`) and a refusal (`<path>: <reason>`), need the path, so the function also takes it. The service and the harness each do their own read, then call it. The CPU work, decoding and diffing, runs on the service's thread, never the core's.
- **`FileOp::Revert { buffer, tick, path, text: Rope }`** carries a rope snapshot, cheap as for `Save`. The reply is `Msg::Reverted { buffer, tick, result }`, where `Revert { edits, crlf, bom, hash }`.
- **Applying (`Buffer::replace(&edits)`):** applies the edits from last to first, so each range is still valid in the text it applies to. Each edit is a delete then an insert, recorded like any edit. Read-only is lifted for the duration, as `append` does it.
  - The handler calls `undo_boundary(point)` before and after, so the revert is its own undo group.
  - It then sets `disk`, `crlf` and `bom` and calls `set_saved`.
- **Stale replies:** when the buffer's tick differs from the request's, or the buffer is gone, the reply is dropped. Its edits are relative to text that no longer exists. Re-requesting is left to auto-revert, which knows when the disk changed.
- **`revert-buffer`** lives in `visiting.rs`, next to `kill_buffer`, and is registered in `builtins.rs` without a key binding. It asks the way `kill_buffer` does, with choices `yes` and `no`, and only when the buffer is modified. It then pushes `FileOp::Revert`. Errors in the reply are echoed.

## Risks / Trade-offs

- [Two reverts in flight: the second reply is stale once the first applies, though its text may be newer] → only `revert-buffer` requests reverts now, one at a time. Auto-revert (#80 onwards) re-reads on each watcher event.
- [The diff of a huge file with many changes is slow] → it runs on the blocking pool and the UI keeps going. `similar`'s Myers is O(ND).
- [Undo after a revert leaves the buffer modified against the new disk hash] → that is correct: the restored text differs from the disk.
