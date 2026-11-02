# Design

## Context

Each block above the prompt is `text\n\n`, a tool call found by a marker and lengths. A fold is `Prop::Invisible` from the header's newline to the block's end. Properties must not nest, or `properties_near` stops bisecting, so a fold over rendered rows can't simply be laid on top. Group membership is only known once a second call arrives, after the first is drawn.

## Goals / Non-Goals

**Goals:** groups cost nothing while folded; everything stays in the rope for search and kill; the same path for live and replayed updates.

**Non-Goals:** question blocks (not built); the frame's clock on a group line, which belongs to the message below; group folding by key from a row with content.

## Decisions

**Rows are joined in the rope.** Members sit one `\n` apart, not hidden blank lines, because `C-n` would stop on a hidden blank. A new block goes in as usual and the newline before it is deleted. When the second call forms the group, the same is done for earlier rows, then the group line is inserted before the first. `Stay` markers at that position (the first row's, a pending ask's) are moved past it by hand.

**A session tracks a run.** `run: Vec<Row>` collects rows until the group forms; `group: Option<usize>` then points into `groups: Vec<Group>`, where the rows move. `Row` is a call's id, a thought or an ask. Ending a run clears both, freeing the markers of an unformed run's rows. Alternative: deriving groups from transcript order on every update. There is no block list to derive from, and it would rewrite more.

**A folded group's hidden rows carry no properties of their own.** It holds only `Invisible` runs from the end of one shown row to the end of the last hidden one, and shown rows render normally, so nothing nests and folded cost is one property per run. Any update to a folded group removes its properties below the line and lays them again (`fold_rows`), which also repairs text appended at a fold's end, where the `Stay` end marker is left behind. An open group renders only the changed row and the line. Opening or folding renders all its rows.

**Row indent is text.** Two spaces start the header, kept by `show_tool` and `render_tool` with an offset. A display-only indent would need a new property, and the header is already plain text.

**Thought rows.** The row's fold mark and indent are text inserted at the thought's end, or when the group forms. A streaming thought renders as before when the group is open, and only extends the fold when folded.

**Asks keep a second marker for their row.** `settle` removes the ask's own marker, and an answered line stays in the group, so a row needs one that outlives it. Whether a row is pending comes from `session.asks`.

**Duration.** `Tool.ended` records the first finish, so the span is `max(ended) - min(arrived)` and includes permission waits, per "first start to last end".

## Risks / Trade-offs

- [Every folded update re-adds the group's fold props] → They're few and streaming terminal output takes the existing fast path, inside the fold.
- [A long-running group's list of rows only grows] → Bounded by the transcript, as `tools` is.
- [Toggling from a row with content can't fold the group] → `TAB` on the group line or a content-free row does. Matches the issue's "TAB on a row opens that call".
