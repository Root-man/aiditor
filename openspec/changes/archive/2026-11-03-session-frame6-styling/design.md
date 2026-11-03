# Design

## Context

The session buffer is one rope rendered by the core into text plus properties (`Prop::Named` faces, `Invisible`, `Line(Decoration)`, `AlignRight`, `Icon`, `Display`). `frame.rs` lays each window out and calls `window_display`, which scrolls the window by point and builds `WindowDisplay { lines, cursor, mode_line, … }`. The frontend (`element.rs`) draws lines from the window's top, offset by `TEXT_INSET` (12 px) or a gutter. Tool-call bodies are markdown rendered by `markdown::render`, which hides a fence's text but keeps its newline, so each fence shows as an empty `session-block` row. The rope's prompt glyph `PROMPT = "> "` is used as a char count in several places through `PROMPT.len()`.

## Goals / Non-Goals

**Goals:**
- Every frame 6 gap in #441 except the permission block, with the rope's text unchanged where possible so copy, search and existing text assertions hold.
- One new display primitive per need: a line indent and a prompt pane. No per-feature special cases in the frontend.

**Non-Goals:**
- The permission block's look and number keys (a separate task).
- Matching the frame's font or exact pixel metrics beyond the stated insets and paddings.
- Scrolling in display rows instead of buffer lines (the existing shortcut in `window_display` stays).

## Decisions

### The glyph is a display string, not text
The `>` of each echo and of the live prompt line gets `Prop::Display(vec![("❯", [Face(StatusInfo), Bold])])`, the mechanism chips already use. The rope keeps `> `.
- Alternative: change `PROMPT` to `"❯ "`. That breaks `PROMPT.len()` byte-as-char arithmetic in about eight places, changes what copy yields and touches every test that asserts `> `. Rejected.

### The user background is a line decoration
Each echo line gets `Line(Decoration::Background(FaceName::SessionUser))`, as code blocks use `SessionBlock`. Replayed user messages go through the same path. No new face.

### Plan lines are rewritten, not decorated
`plan_line` returns `  <mark> <content>` and the block drops `render_rows`' rule. The `(status, priority)` suffix goes. Faces: the mark and content in `session-thought` for completed and pending, the mark in `status-waiting` for in-progress. `plan_text` builds the block's text and `render_plan` its faces, replacing `plan_rows`; in-place replacement stays as it is.

### Diff bodies show hunks only
`unified()` stops calling `.header(path, path)` and the content item drops its `{path}\n` line. `similar`'s unified diff without a header still starts each hunk at `@@`, so `diff_faces` keeps working, with its `i < 2` header branch removed. `tool_output` (the `RET` buffer) keeps the full header, since a standalone diff without paths loses context. The `+N −M` counts come from `similar::TextDiff::from_lines(old, new).iter_all_changes()`, summed over the call's diff items, computed when the header is built.

### Fences hide with their newline in tool bodies only
`markdown::render` gets a flag (as it already takes `thought`) that makes a fenced block's delimiter lines `Invisible` together with the newline joining them to the block: the opening fence and its trailing `\n`, and the closing fence with the `\n` before it. An invisible run spanning a newline already merges display lines (folds work this way), so the fence takes no row. Agent messages keep today's behaviour, where a fence is an empty block row. `render_tool` passes the flag. Properties must still not nest, so `hidden_output`'s cut is laid out against the new ranges: the closing fence's run starts where the cut's ends.
- Alternative: drop fences from tool-body text. That changes copy and kill output and the markdown parse. Rejected.

### A line indent decoration
`Decoration::Indent(usize)` on each open body line, set by `render_tool` to the header's status column plus two (4 for an ungrouped call, 6 for a grouped row). `display::build` subtracts the indent from that line's wrap width and records it on `Line` (`indent: usize`). The frontend offsets the line's text, icons, boxes, cursor and `Background` decorations by `indent * cell_width` and ends an indented background at the right text inset. `Rule` and `Bar` stay at the left edge. The cursor stays a char column in the line's text.
- Alternative: leading spaces in the rope. Four or more spaces turn a fence into an indented code block in markdown, and copy would carry them. Rejected.

### Session windows get their own inset
`frame()` already knows when a window shows a session (it swaps in the session mode line). For those windows the text starts at 27 px and `cols` is computed from `rect.width - 27 - 24`. The frame display reports the text start per window, as `FrameWindow` with a gutter already does, so `element.rs` reads it instead of the constant. Non-session windows keep `TEXT_INSET`.

### The prompt pane is a second display of the same window
For a session window, `frame()` splits the window's text rows:
1. `prompt_line` is the buffer line holding `prompt_start() - PROMPT.chars().count()`. The pane's rows are the prompt area's wrapped row count (from `Window::row_starts`), clamped to `1..=rows / 3`.
2. The pane's height is its rows plus a 1 px rule and 16 px of padding. The transcript gets `text_rows(rect.height - pane_height)` rows.
3. Transcript: if point is in the prompt area, `top = prompt_line.saturating_sub(transcript_rows)` (the last lines before the prompt). Otherwise the window's own `scroll` with `rows = transcript_rows` runs, and `drawn` is capped so no row reaches `prompt_line`.
4. Pane: `display::build` from `prompt_line`, with a top that keeps point's row visible when point is in the prompt. Otherwise the top is `prompt_line`. The cursor is passed only to the part that holds point.

`WindowDisplay` gains `prompt: Option<PromptPane { lines, cursor, cursor_width }>`. The frontend draws `display.lines` from the top as today, then the rule and the pane's lines anchored to the mode line's top. The message bar, `hl_line` and right-aligned times work per `Line`, so both parts get them unchanged.
- Alternative: two real windows in the split tree. That needs a second buffer or narrowing, breaks `other-window` and `C-x 1` semantics, and gives the prompt its own point. Rejected.
- Alternative: no docking, inline prompt. This is what we have today; frame 6 and the user chose docking.

### Faces for header targets and the count
The command target gets `Named([Face(SessionCode)])` in `render_tool`, and the `+N`/`−M` runs get `status-running` and `status-failed`, the faces of the status marks, which frame 6 uses for them (its `ok` and `bad` classes); the diff faces carry a background. `… N more lines` is the placeholder of a `Prop::Invisible`. The first step is to check how `display::build` faces a placeholder span. If it takes the faces covering the hidden range's start, a `Named([Face(Annotation)])` run over that range is enough. Otherwise the placeholder drawing takes the `annotation` face for every non-empty placeholder. The only other placeholder is a folded call's ` …`, which frame 6 also draws dim.

## Risks / Trade-offs

- [Split display breaks scroll commands (`C-v`, `M-v`, `C-l`) that assume one row count] → They read `Window::rows`, which `scroll` sets. The transcript call sets it to the transcript's rows, so they page the transcript. A test covers `C-v` with point in the transcript.
- [A fence run that spans a newline next to `hidden_output`'s cut could nest properties] → Lay out the cut first and clip the fence's run at the cut's end. A core test opens a 50-line command and checks `properties_near` stays sorted.
- [Indent shrinks wrap width per line, which `row_starts` must agree with] → `row_starts` takes the same per-line width from the indent decoration, and the existing wrap tests gain an indented case.
- [The prompt pane at a third of the window can hide a long draft's start] → The pane scrolls to keep point. `shortcut: pane top is recomputed from point each frame, not remembered; keep a pane top on the window if it jumps while editing long prompts.`
- [Existing tests assert the plan's `(status, priority)` text, diff paths and fence rows] → Update them in the same task as each change, as the deltas specify.

## Migration Plan

Display only. No settings, data or protocol change. Rollback is a revert.
