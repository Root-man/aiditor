# Proposal

## Why

The 2026-11-09 refactor scan of the 52 M6 commits after #404 (`34f6a0b..0f74a9b`), read against the 2026-11-08 full benchmark run, found one severe slowdown and several smaller costs and design problems in the agent session buffer and the display builder. The severe one: once any property is added out of order, a session buffer never uses the sorted property lookup again. A question block or a thought inside a tool group triggers it, so ordinary sessions hit it. A probe measured a clean frame of a 5,000-line transcript at 0.21–0.31 ms sorted and 51–55 ms unsorted. The M7 rich session and the M8 session rework build on this code next.

## What Changes

Each finding keeps its number from the scan.

- **#1 Property lookup in any insertion order:** keep an always-sorted index beside the stored properties, so `properties_near` bisects whatever order properties were added in. Stored (layering) order stays as it is today.
- **#2 Icon lookup per build:** stop scanning every nearby property for icons on each char of `display::build`. Look up each line's background face once per line, not once per char.
- **#3 Open tool calls append their output:** a chunk for an open command call appends to the hidden part past the 20 shown lines and rewrites the `… N more lines` count, instead of rebuilding the whole call.
- **#4 Workspace list out of the generic display:** move the workspace list's mode line and its selected-row background from `display.rs` into `workspace_list.rs`, called from `frame.rs` the way session windows are.
- **#5 Streamed messages re-render their tail:** re-render a streaming message from the start of its last top-level markdown block at the previous render. Everything before that point keeps its text and properties. The result must equal a full render.
- **#6 Session block extents and lookup:** replace the hand-kept `len` of each block (messages, tools, asks, answered questions, thoughts, plan, working line) with end markers. RET and TAB find the block at point with one lookup instead of a chain of per-kind probes.
- **#7 `has_folds` without a scan:** keep a count of fold properties instead of scanning every property on each call.
- **#8 Session lookup helper:** one helper that finds a session and its buffer, replacing 7 repeated lookup blocks and 8 `expect("found above")` re-lookups.
- **#9 No theme clone per frame:** stop cloning the whole `Theme` in `prepaint` and `paint`.
- **#11 Per-line property queries without allocation:** `table_line`, `line_indent` and `unfolded_line` iterate the nearby properties instead of collecting them into a `Vec`.
- **#12 `echo_area` split:** move the completion-candidate row out of the 200-line `echo_area` into its own function.
- **#10 is dropped:** on inspection, moving the `<workspace>` suffix strip into `display::mode_line` adds a parameter to the 10-parameter `display::build` and deletes nothing. The strip in `frame.rs` stays.
- **Measurements and records:** rerun the affected benchmarks, update `docs/performance.md`, correct the 2026-11-08 report's "unchanged benchmark sources" claim (#457 added a table to the `hot-stream` fixture), and record the scan in `docs/refactoring-opportunities.md`.

No behavior changes. Every finding keeps the observable text, faces, folds, motion and commands as they are, so the change sets `skip_specs`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. The existing `text-buffers`, `display-model` and `session-buffers` requirements already describe the behavior this change keeps.

## Impact

- Core (`aiditor-core`): `buffer.rs` (property index, fold count, iterating per-line queries), `display.rs`, `frame.rs`, `window.rs`, `workspace_list.rs`, `agents.rs`, `agents/render.rs`, `agents/groups.rs`, `agents/questions.rs`, `agents/pastes.rs`, `markdown.rs`, and nearby tests.
- Frontend (`aiditor`): `element.rs` (theme borrow) and `editor.rs` (`echo_area` split).
- Benchmarks: a new focused `hot_paths` case for a session with unsorted properties. Existing benchmark names, fixtures and sampling stay unchanged.
- Docs: `docs/performance.md`, `docs/measurements/` (a new run and the 2026-11-08 report correction), `docs/refactoring-opportunities.md`. `docs/editor.md` only if the implementation contradicts it.
- No new dependencies, settings, commands, public API or data formats. Not in scope: the M8 session rework's functional or visual changes, and the unexplained part of the copy-mode scroll regression (4.17 vs 3.12 ms).
