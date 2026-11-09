# Design

## Context

See proposal.md for the findings and why they matter. The state this design builds on:

- `Buffer` stores properties as `Vec<(MarkerId, MarkerId, Prop)>` plus a `props_sorted` flag (`buffer.rs:767-840`). While the flag holds, `stored_properties_near` bisects by start and by end, which needs both starts and ends to ascend. `add_property` clears the flag as soon as a property would break that order. Only `set_copy_text` sets it again, so a session buffer stays on the full-scan path once cleared.
- The stored order is also the layering order. `display::build`'s `face_at` applies faces in the order `properties_near` returns them, and fold detection takes the first match. In sorted mode a property goes to its sorted place; in unsorted mode it is appended. Today, buffers that went unsorted layer in add order.
- Marker positions move monotonically under edits. Every start marker is `Advance` and every end marker is `Stay`, so edits preserve any order among starts, and among ends.
- Session blocks keep a start marker plus a char length (`Message.len`, `Tool.len`/`header`/`meta`, `Ask.len`, `Answered.len`, `Thought.len`, `Turn.len`, the plan's `(MarkerId, usize)`). Pastes already keep start and end markers (`agents/pastes.rs`).
- A streamed message keeps its markdown in `Stream.source`. `render_message` re-parses all of it with tree-sitter-md and rewrites the block's text with a line diff (`agents/render.rs:29-53`). Drawn tables replace their source in the text, so the text and the source differ inside tables.
- An open command call shows 20 output lines, then a `… N more lines` count. The rest stays in the text under an `Invisible` fold (`render_tool`, `hidden_output`). `append_tool_output` only handles folded calls; open calls go through `show_tool`, which rebuilds the content from the whole terminal output.

## Goals / Non-Goals

**Goals:**
- A property lookup costs about the same whatever order the properties were added in, and layering stays exactly as it is today.
- The per-frame and per-chunk session costs found by the scan no longer grow with the size of the transcript or the output.
- Each finding has a test proving the visible result is unchanged, plus a measurement where it is a performance fix.

**Non-Goals:**
- An interval tree or any other new data structure for properties beyond one sorted index.
- Making the markdown parser incremental (tree-sitter `Tree::edit`); re-rendering a shorter tail is enough.
- Moving `workspace_list`/`workspace_rows` off `Buffer`, and storing workspace-list state the way sessions do.
- Profiling the remaining copy-mode scroll regression.

## Decisions

### D1 · A sorted index beside the stored properties (#1)

`props` stays in its current order, which keeps today's layering. Beside it, `Buffer` keeps:
- `by_start: Vec<u32>`, the indices of `props` sorted by start position;
- `reach: Vec<u32>`, where `reach[k]` is the index (into `props`) of the largest end among `by_start[..=k]`.

`stored_properties_near(range)` finds `to` as the first `by_start` slot whose start is past `range.end`. It finds `from` as the first slot whose `reach` end is at or past `range.start`; `reach` ends never decrease, so a binary search works. It then collects the touching properties in `by_start[from..to]` and sorts the (few) indices it found, returning them in stored order.

Monotonic marker movement keeps `by_start` sorted and `reach` valid under edits. A tie between starts stays a tie, and a tie between ends stays a tie, so `reach`'s argmax stays a maximum. `add_property` inserts the new index into `by_start` at its place and recomputes `reach` from there. `remove_properties` and `set_copy_text` rebuild both, as they already walk every property. `props_sorted` and the fallback scan go. The `render_tool` and `user_lines` comments about keeping properties sorted become obsolete; the code they describe stays.

Alternatives:
- Sort `props` by start and layer by a sequence number: changes the stored order every consumer sees, so layering would need checking everywhere.
- Re-sort on demand: nested properties never have ascending ends, so a re-sort cannot bring back the two bisects.
- An interval tree: more code for no measured gain over one bisect and a short scan.

### D2 · Icons and line faces found once (#2)

Before the char loop, `build` collects the window's icon starts, as a `Vec<(usize, Icon)>` sorted by position, from the properties it already gathered. Each char then checks a moving cursor into that list instead of running `find_map` over every nearby property. The `line_faces` lookup moves from per char to per line: it is computed when `line.number` changes and reused for that line's chars. The other per-char work (folds, faces) is unchanged; it existed before #404.

### D3 · Appending to an open command call (#3)

`append_tool_output` also handles an open call, but only once its output is already past the 20-line cut (`hidden_output` returns a range). Then it:
1. inserts the tail before the closing fence, as for a folded call;
2. extends `tool.faces` the same way and adds the tail's SGR face properties, which sit inside the hidden range;
3. extends the cut's `Invisible` fold to the new end;
4. rewrites only the count line when the hidden line count changed.

Below the cut, and in every other case (fence growth, exit, an update with fields), it falls back to `show_tool`, which is cheap while the output is short. `TermRender` gains the hidden line count and the char range of the count line, so the count can be rewritten without re-scanning the output. The existing "open and folded agree" and "streams past the cut" tests cover the result; a new test compares the text and every property after appending against a fresh `render_tool`.

Alternative: render only the visible 20 lines and keep the rest out of the text. That changes what copying and search see (both include the hidden output today), so it is rejected.

### D4 · Workspace-list display in its own module (#4)

The `workspace-list-mode` mode line and the selected-row `CompletionSelected` background leave `display::mode_line` and `display::build` and become `Snapshot` methods in `workspace_list.rs`. `frame.rs::window_display` calls them right after `snapshot.fit`, where it already handles the list. `display.rs` stops mentioning the list. The mode line keeps its text and faces; the existing frame tests (`*Workspaces* │ 1 projects, 1 workspaces  1/2`) check that.

### D5 · Re-rendering the tail of a streamed message (#5)

`markdown::render` already parses the whole message. It now also reports the stable point: the source byte offset of the last top-level block (a child of `document`, or of a `section`, which tree-sitter-md opens for each heading), with that block's position in the rendered text. `Stream` stores `(source_byte, text_char)` from the previous render. The next render parses `source[source_byte..]` and rewrites only `start + text_char..end`, shifting the tail's properties by the prefix. Earlier blocks keep their text and properties.

This is safe because no later chunk can change how an earlier top-level block renders in this renderer:
- a list that keeps growing, a lazy continuation, and a setext underline all belong to the last block, which is always re-rendered;
- the renderer has no reference links;
- tables are drawn per top-level block.

Links are found per render as today (the existing `shortcut:` comment still applies). `trim_message` and `end_stream` operate on the tail. A test streams a message containing a heading, a list growing across chunks, a fenced block, a table and a setext heading in many chunk splittings, and checks after each frame that the text and properties equal a full `render_message` of the same source.

Alternative: tree-sitter incremental parsing with `Tree::edit`. It saves the parse but not the rewrite and property rebuild, which the diff and `remove_properties` make O(message), and it does not support tables drawn into the text. Rejected for this change.

### D6 · Block ends as markers and one block lookup (#6)

Each block's extent becomes a pair of markers: the start it already has, plus a `Stay` end marker. This is the precedent `Paste` already sets. `Stay` keeps `put_meta`'s line, inserted after the block's newline, out of the block. Sites that grow a block at its end (streaming in `output` and `show_user`, `append_tool_output`, `retime` in `tick_session`, a `show_tool` rewrite) set the end marker to the new end with `set_marker`, instead of adding to a length. Offsets inside a block that are relative to its start (`header`, `label`, `outcome`, `first`, `meta`) stay as they are; only extents become markers. Removing a block removes both markers.

RET (`agent_send_input`) and TAB (`agent_toggle_tool_call`) each start with one `Session::block_at(buffer, point) -> Option<At>`. `At` is an enum: `Ask { at, line }`, `Answer`, `Paste`, `Tool(id)`, `Thought`, `Group`. Each command matches on it in the precedence it has today (an ask first, then paste, answer, tool, thought, group). Chips, URLs and links are inline in messages rather than blocks, so they keep their lookups in RET.

Alternative: one `Vec<Block>` holding every kind in transcript order. That is a bigger rework, best decided by `m8-session-rework`, so this change keeps the existing collections.

### D7 · Small ones (#7, #8, #9, #11, #12)

- **#7:** `Buffer` keeps `folds: usize`, raised by `add_property` for `Invisible`/`Display` and lowered by `remove_properties` and `set_copy_text`. `has_folds` reads it. A property deleted to an empty range still counts, as it does today.
- **#8:** `Core::session_buffer(&mut self, id) -> Option<(&mut Session, &mut Buffer)>` borrows the two fields separately. Each `let Some(session) … let Some(buffer) …` pair and each `expect("found above")` re-lookup becomes one call.
- **#9:** `prepaint` and `paint` take the faces they need from a borrowed theme. Where a borrow of `cx` blocks that, they copy only the needed `Face`/colour values, which are `Copy`, instead of cloning the maps.
- **#11:** `stored_properties_near` gets an iterator form used by `table_line`, `line_indent` and `unfolded_line`; `properties_near` keeps returning a `Vec` for its other callers. Because D1 returns stored order by sorting the found indices, the iterator form is for callers that ignore order: a `find`/`any` over a kind.
- **#12:** the candidate-row closure in `editor.rs::echo_area` becomes a function `completion_row(...)`. Markup and debug selectors stay the same, which the existing GPUI tests check.

## Risks / Trade-offs

- [D1 index drifts from `props` after an edit path that changes markers without going through `add_property`/`remove_properties`] → A debug assertion in `stored_properties_near` checks that `by_start` is sorted and `reach` is valid in tests. A property test adds random nested ranges, edits and removals, and compares `properties_near` with a brute-force scan.
- [D1's sort of the found indices costs more for windows touching many properties, as in terminal copy mode] → `k` is bounded by the window. The `terminal-copy` benchmarks catch it.
- [D5's stable point is wrong for a markdown shape the reasoning missed] → The equivalence test runs many chunk splittings. In debug builds, `render_stream` also compares the tail render with a full render.
- [D6's `Stay` end marker isn't moved at a growth site, so a block's extent is short] → Tests per block kind assert the extent after streaming, rewrites, retiming and meta lines. Since `len` is no longer kept by hand, a missed site shows up as a failing extent assertion instead of a silent drift.
- [D6 grows into the M8 rework] → It is the last task section and touches only extents and the RET/TAB dispatch; anything beyond that waits for `m8-decide-session-rework`.
- [D3's fallback condition is wrong] → Without an existing cut, the fast path is never taken. The comparison test with a fresh render covers it.

## Migration Plan

All core state, no persisted formats. Each finding is a separate commit in the order of tasks.md, so a regression can be reverted alone. No rollback beyond `git revert`.
