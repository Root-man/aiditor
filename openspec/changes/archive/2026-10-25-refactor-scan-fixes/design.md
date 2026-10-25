# Design

## Context

See proposal.md for why. Each item was checked against the code at `1a4df61`; the findings that shape the approach:

- `Ctx::end_terminal` removes the terminal from `core.terminals` before killing its buffer, so `sync_terminals` never emits `TerminalOp::Kill`. The service's `Handle::hang_up` already returns no groups for `Shell::Exited` or `Shell::Starting`, and its thread's failed-start path leaves the state at `Starting`. So a `Kill` after exit or a failed spawn sends no signal. It only drops the handle: the master PTY and the input sender, which ends the input thread.
- `recover_buffers` does `delete(..).expect` then `insert(..).expect`, which refuse a read-only buffer. `Buffer::replace` already edits past read-only as one undo group, and has a test.
- The recovery worker runs ops in order and logs failures, so a failed `AutoSave` is followed by the `DeleteRecovery` queued after it.
- In production, `add_property` is called only by `vterm_copy_mode`, which adds faces sorted by start and non-overlapping. Folds and line decorations only appear in tests today, but `display::build` must keep handling them.
- `display::build` walks the hidden columns of point's line one char at a time, checking for a fold at each. It also highlights `first..last`, the whole buffer lines, even where only `shown` columns are drawn.
- `Core` is single-threaded and buffers already hold `Rc<RefCell<..>>` (terminal replies), so interior mutability in a buffer is acceptable.

## Goals / Non-Goals

**Goals:**
- Fix each item at its root, in the shared code, reusing the existing operations (`TerminalOp::Kill`, `Buffer::replace`, the batching clock).
- Keep `display::build`'s output identical: the existing display tests are the regression check for A3, B2 and B4.

**Non-Goals:**
- No new `TerminalOp`, message or acknowledgement channel.
- No interval tree or general property index; no change to marker storage. The `markers` walk-on-edit shortcut stays.
- No GUI benchmark or Linux/macOS re-measurement beyond the core and service benches touched.

## Decisions

**A1: reuse `Kill` as the release.** `end_terminal` pushes `Effect::Terminal(TerminalOp::Kill { id })` once it removes the id, whether or not the buffer is still live. Alternative: a new `TerminalOp::Forget`. Rejected: `Kill` on an exited or unstarted shell already does exactly that, and `Exited` is set before the exit message is sent, so there is no race in which `Kill` signals a reaped PID. The doc comment on `end_terminal` changes from "so `sync_terminals` sends no kill" to sending it here, exactly once.

**A2: `buffer.replace(&[(0..len, r.text.clone())])`** replaces the delete-insert pair. It stays undoable and passes read-only.

**A6: the replacement carries the file it supersedes.** `RecoveryOp::AutoSave` gets `replaces: Option<PathBuf>`. `write_autosaves` leaves it `None`. In `recover_buffers`, after `write_autosaves`, set it on the `AutoSave` effect just queued for the recovered buffer. Push `DeleteRecovery` only when no such effect exists: the buffer with no file, or `keep` false. The worker removes `replaces` only after `fs::rename` succeeds, applying the same own-pid guard as `DeleteRecovery`, which moves into one shared helper. Alternative: the doc's ack message and retry. Rejected: a failed regular auto-save keeps the previous recovery file, so only this path can lose data, and an ack crosses the service/core boundary for no added safety.

**A3: skip the hidden prefix, narrow highlight ranges.**
- In `build`, while `col < range.start`, jump `pos` to the nearest of: the start of the visible columns, the line's newline, or the next `Invisible` start in between. This mirrors the existing `col >= range.end` skip. With no folds the jump is direct, and the fold check runs per jump, not per char.
- Highlights are requested per displayed buffer line, for that line's drawn columns only: `shown` on point's line, `0..shown.len()` on the others, clamped to the line. Syntax context is kept because the query still matches the nodes overlapping each range. `Syntax::highlights` takes a list of char ranges and runs one `QueryCursor` per range, reusing the colours vector. The runs come back concatenated, still sorted and disjoint. Painting covers only those ranges.
- Folds: the window's lines joined by a fold are not highlighted. That is today's documented shortcut, unchanged.

**A4:** `copy_text` keeps `offset` as a running char count of the current logical line. It adds each appended row's char count and resets to 0 on a hard newline, instead of calling `current.chars().count()` every row.

**A7:** `char_idx` reads the last char with `slice.get_char(len.wrapping_sub(1))`, or an equivalent guarded index, instead of `chars().last()`.

**B1:** initialise `sent` from one `now()` call as `t.checked_sub(BATCH).unwrap_or(t)`. One call matters because the test clock advances on every call. Expected batches in `one_ongoing_file_streams_ordered_batches_on_the_deadline` become `[(1, false), (2, false), (1, false), (0, true)]`.

**B2: a sorted-disjoint flag with binary search.** `Buffer` gains `props_disjoint: bool`, true while every property starts at or after the previous one's end, in insertion order. `add_property` clears it when the new range starts before the last property's end. `set_copy_text` resets it with the props. This holds under edits because marker positions map monotonically, and an end `Stay` never passes a start `Advance` at the same position. A new `Buffer::properties_near(first..last)` binary-searches both bounds when the flag is set, and falls back to the current full scan otherwise. `build` still takes every property when any `Invisible` exists. It checks that by matching the stored `Prop` variants, with no marker lookups, which is cheap even for copy mode's tens of thousands of faces. Alternatives:
- Sort by start alone: wrong for a long property overlapping the window's start.
- Index markers in a `Vec`: a bigger change to edits and removal.

**B4: a one-entry cache in `Syntax`.** `RefCell<Option<(Vec<Range<usize>>, BTreeMap<String, u32>, Vec<(Range<usize>, u32)>)>>`, keyed by the requested ranges and the palette, compared by value. `Syntax::edit` clears it. `highlights` returns a clone of the cached runs on a hit. Alternative: fill the cache from `window_display`'s `&mut` reparse step. Rejected: `window_display` would have to duplicate `build`'s range computation, and the bench calls `build` directly. A `RefCell` keeps `build`'s signature and costs one field.

**B5:** in `scenarios::source`, wrap every 100 generated `rs` items in `mod m{k} { … }` and every 100 `tsx` items in a component. Only the 100k cases matter, but one generator serves both sizes. `docs/performance.md` notes that `syntax-load`, `syntax-edit-frame` and `revert-apply-frame` results for `rs`/`tsx` from before this change are not comparable. It also notes that `grep/first-batch` now measures time to the first hit.

## Risks / Trade-offs

- [A1: `Kill` after exit with a background job holding the PTY] → `hang_up` sends nothing, and dropping the master hangs up the tty as quit already does today. That is the same behaviour as `kill_all`.
- [B2: a future producer adds overlapping properties] → the flag drops to the full scan. That is correct, just slower. Covered by a test with an overlapping property.
- [B4: palette compared every frame] → about 40 string comparisons. It is cheaper than a hash and survives settings reloads.
- [A3: one query cursor per line] → about 50 per frame. If the per-pane benchmark regresses, merge adjacent ranges whose lines are short, since they are contiguous without hscroll.
- [B5: the benchmark baseline shifts] → CI compares the same scenario names, so the first run after this change shows a step. Recorded in `docs/performance.md`.
