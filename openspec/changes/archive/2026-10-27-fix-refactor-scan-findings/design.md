## Context

Six independent fixes, each confirmed against the code at `155539d` (see proposal.md - Why). They touch the display builder, markdown rendering, the agent service, settings reload and the file prompt. They share no code, so each section below stands alone, and tasks are grouped the same way.

Facts the approach depends on:

- `display::build` lays out the window's top line from column 0 through `(skip + rows + 1) * cols`. `wrap_lines` then splits every row and drops the first `skip` (`display.rs:465-468`). The soft-wrap design accepted this cost as a risk. `display::row_starts` is greedy: each break depends only on the previous row's start, the line's length and which chars are whitespace.
- `markdown::render` composes properties by scanning every hidden range and face, and every code line, for each piece between cuts (`markdown.rs:133-161`). `render_stream` runs it over the whole streamed block once per frame.
- The agent service's `connect` returns when the agent closes stdout and clears the connection. Prompt and cancel ops then return early at `self.connection(id)?`. The exit task only reports once stderr closes and `wait()` returns. The core's `agent_exited` already ends the turn, settles asks and kills.
- Session properties hold concrete `u32` colours (`Face.fg/bg`, `Decoration::Background/Rule`). `Msg::SettingsLoaded` just replaces `core.settings`, and `Theme` is `PartialEq`.
- `on_listed` accepts a listing whose directory equals the awaited one and `!listed`. The preview path already tags requests with a counter (`preview_revision`).
- The benchmark scenarios are written in order inside one binary, so one failing `assert!` ends the whole run.

## Goals / Non-Goals

**Goals:**
- Make each fix the smallest one that removes the confirmed problem.
- Keep rendered output identical wherever the spec doesn't change it.
- Add a test or bench that would have caught each problem.

**Non-Goals:**
- Making the per-frame cost of re-rendering a streamed block independent of its length (re-parsing is linear and stays).
- Caching row starts for `Window::scroll`.
- Storing theme names in properties instead of colours.
- Retheming terminal copy-mode faces.
- Timing out an agent's stuck stderr.

## Decisions

### 1. Benchmarks: truncate the old case, add the wrapped one
`syntax-display/rs/1mb-single-line-at-end` runs `toggle-truncate-lines` before `move-end-of-line`, so it keeps its name, its assertion and its history in `scripts/benchmarks.py` comparisons.

A new `syntax-display/rs/1mb-single-line-wrapped-at-end` asserts that the row holding the cursor ends in `xx";` and is highlighted. This is the case that #1 speeds up.

A new `session-render/5000-line-styled-message` drives a session through the public `Msg` API: `agent-start`'s effect, then `Msg::AgentStarted`, then one `Msg::AgentUpdate` chunk holding 5,000 lines with headings, emphasis, inline code and fenced blocks. It measures the frame that renders that chunk.

Alternative: rewrite the old case to wrap. That loses its comparable history and still leaves the truncated path unmeasured.

### 2. Markdown: sweep the cuts with an open set
Precompute, for each cut, the faces, hidden ranges and code lines that start there and the ones that end there. Empty ranges are dropped first, since the old `covers` test never matched them.

Walk the cuts in order and keep:
- the open faces in a `BTreeMap` keyed by push index, so folding them in key order gives exactly the old "later (inner) faces win" result;
- a count of open hidden ranges;
- whether a code line starts at this cut.

A piece is covered by a range exactly when the range is open at the piece's start, because every range boundary is a cut. So the open set is the old filter, and the cost drops from pieces × ranges to roughly pieces × nesting depth. The joining rule and the byte-to-char conversion stay as they are.

Alternative: an interval tree. It is more code for the same result, given that nesting is shallow.

### 3. Wrapped layout starts at the window's first row
`display::Wrap.skip` (a row count) becomes `from`, the char column in the top line where the window's first row starts. `window_display` gets it from `window.row_starts(buffer, top)[window.top_row()]`. `Window` already computes these starts for scrolling, so this adds one more call on the same line.

`build` starts the top line at `line_to_char(top) + from`. That offset also applies to `first`, the `properties_near` window, and the top line's `drawn` highlight range. The top line's columns count from `from` as if it began there. `shown` for a wrapped window becomes `0..(rows + 1) * cols`, and `wrap_lines` no longer drops rows.

Since `row_starts` is greedy, wrapping the suffix from a row start yields the same breaks as wrapping the whole line and then dropping the earlier rows. The test compares the two.

Alternative: walking whole rows inside the char loop. The soft-wrap design rejected this because it tangles folds and hscroll.

### 4. A closed connection terminates the group
`connect` takes the agent's pid and `exited` flag. When `connect_with` returns after `started`, it does what `AgentOp::Kill` does: `killpg(SIGTERM)` unless `exited` is set, then SIGKILL after `GRACE` on a thread. Factor that into one function that both call.

The group dying closes stderr, so the existing exit task then reaps the agent and sends `AgentExited` as usual. The core needs no change, because `agent_exited` already ends the turn. When the agent closed stdout because it exited, the signal goes to a group that is gone or a zombie leader's group, which is harmless.

Alternatives:
- A new `Msg::AgentDisconnected` for the core. That is more surface for the same end state, and it leaves the process running.
- `child.start_kill()`. It misses grandchildren that hold stderr.

### 5. Retheme session buffers on reload
On `Msg::SettingsLoaded`, when the parsed theme differs from the current one, build `(old, new)` colour pairs for `dim`, `code_block`, and `syntax::color` of `text.title`, `text.literal` and `text.uri`. Skip pairs where the old colour is missing or the two are equal. Then, for every session in `core.agents`, a new `Buffer::map_colors(f)` rewrites each property's `Face.fg`, `Face.bg` and decoration colour through the pairs. No re-render: the blocks keep their ranges, folds and hidden markup, and the stream marker is unaffected.

Alternatives:
- Re-render every block. Completed blocks have no stored boundaries or kinds (message, thought, tool, plan, ask), so this would need a block index.
- Store theme keys in `Face`. That touches every face consumer and the frontend.

### 6. Listings carry a request id
`Prompts` gains a `listings: u64` counter. `FileOp::ListDir(dir)` becomes `ListDir { dir, request }`, and `Msg::Listed { dir, result }` becomes `Listed { request, result }`. `dir` was only used for matching.

`Completion.listed: bool` becomes `awaiting: Option<u64>`. Each listing push bumps the counter and stores it there. `on_listed` accepts only `Some(request)` and `take()`s it. Because the counter is never reset, an id from a closed prompt can never match a new one.

The files service echoes the id, and the harness passes it through. This follows the `preview_revision` pattern.

## Risks / Trade-offs

- [Two remapped names share an old colour but get different new ones] → The first pair wins and the other block keeps the wrong colour. This is noted in the code as a shortcut. Bundled themes don't collide.
- [The old theme lacked a `text.*` colour and the new one defines it] → Those headings or links stay uncoloured until re-rendered. The spec states this.
- [The pid is reused within the grace after the agent exits on its own] → This is the same accepted shortcut as `Handle::terminate`, and the `exited` flag guards against it.
- [A fold before the window's first row on a wrapped top line] → Rows are already counted in buffer chars (an existing shortcut), and `from` is a buffer column, so the two stay consistent.
- [The sweep differs from the old output in some corner case] → The existing markdown tests run unchanged. A new test nests a link inside emphasis inside a heading in a thought, with a code block beside it, and checks precedence.

## Migration Plan

None. There is no persisted state or setting. `FileOp` and `Msg` are internal enums, and every producer and consumer is updated in the same change.
