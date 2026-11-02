# Design

## Context

A call's block is rope text with properties, rewritten by `show_tool` and rendered by `render_tool`. Terminal output goes through a folded fast path that appends only the tail (`append_tool_output`), and the display jumps across folds, as measured in `docs/editor.md`. Permission and tick code address the header by its length, with the time as its last part. The design canvas isn't readable from here, so spacing and colours follow `docs/editor.md`'s Tool groups and Times notes; the hand check compares with frame 6.

## Goals / Non-Goals

**Non-Goals:**
- Tool groups and file links: their own tasks. Target paths are plain text here.
- `RET` on the `… N more lines` line, or anywhere but the header.
- A live output buffer: it is a snapshot, refreshed by the next `RET`.
- Capping read, search, fetch or other calls.

## Decisions

**The cut is a fold, not a truncation.** The whole output stays in the rope and `Invisible("… N more lines")` hides everything past line 20. Search, kill and the transcript's "markup stays in the text" rule hold, and the fast path and its benchmarks stay as they are. Alternative: keep only 20 lines in the rope, which would delete the fast path but make search miss output and need a line count kept per terminal.

**The cut is worked out in `render_tool`, only when open.** The open path already slices the body; a folded call needs nothing. `tool_content` reports where the output starts in the body, and the closing fence is always the last line, so the fast path's offsets still hold.

**One fenced block for `$ <command>` and its output.** It reads like a terminal and a command's `*` or backticks can't be taken for markdown. The fence outgrows backticks in the command too. Output stays the last part of the content.

**Markdown runs on the visible text only.** Props past the cut are shifted over it, and faces in the hidden tail dropped or clipped, because a fold's properties must not nest (`properties_near` bisects). It also bounds the open call's markdown cost.

**Header text is plain.** Fixed columns: fold 0, status 2, icon 4..6, verb from 7, then the target. The title's markdown goes; a title used as the target loses surrounding backticks. The fold marker is rope text, changed by `TAB`.

**The target by kind.** A command's `rawInput.command`; a fetch's `rawInput.url`; for read, edit, delete and move, the first location, else the first diff's path, relative to the primary root; otherwise, and whenever those are missing, the title's first line. Search keeps its title since a location would lose the pattern.

**Verbs by status.** A finished call reads past (`Ran`), a pending or running one present (`Running`), matching the rule in the Tool groups note.

**Right side order:** waiting counter, then exit or signal, then duration. Exit is in `status-failed`.

**Full output from the structured call, not the rope:** the terminal's text and faces, else the diffs, else `rawOutput`, else text items. One buffer per call, kept in `Tool` and reused; a killed one is recreated. It belongs to the current workspace and shows with `display_buffer`, as `*Diff*` does.

## Risks / Trade-offs

- [Search can land in a hidden tail] → As it can in a fold today; `TAB` folds from there.
- [An adapter's `Read File` title before its location arrives reads `Reading Read File`] → It lasts one update; left alone.
- [The placeholder draws in the default face] → `Invisible` has no face; acceptable, as ` …` was.
