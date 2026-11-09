# Tasks

## 1. Baseline

- [x] 1.1 Add a `hot-stream/unsorted-props-clean-frame` case to `crates/aiditor-core/benches/hot_paths.rs`: the existing `session()` transcript plus a thought row inside a tool group, so its properties are no longer sorted. Verify the case asserts the frame shows the thought row, and record release baseline medians for it, `hot-stream/*`, `terminal-copy/*`, `display/*`, `frame/*` and `syntax-display/*` with host metadata in a new `docs/measurements/<date>-scan-fixes/` folder. Existing case names, fixtures and sampling stay unchanged.

## 2. Property lookup in any insertion order (#1, D1)

- [x] 2.1 Add `by_start` and `reach` to `Buffer`, maintained by `add_property`, `remove_properties` and `set_copy_text`. Make `stored_properties_near` bisect them and return the touching properties in stored order; remove `props_sorted` and the full-scan fallback. Verify `nested_properties_with_ascending_ends_bisect` and the existing `buffer.rs` property tests pass, and update the tests that assert `props_sorted`.
- [x] 2.2 Add a proptest in `buffer.rs`: random nested and overlapping properties added in random order, interleaved with inserts, deletes and `remove_properties`. Verify `properties_near` equals a brute-force filter of `properties()`, same order, for random ranges. Add a debug assertion of the index invariants inside `stored_properties_near`.
- [x] 2.3 Run `cargo test -p aiditor-core` and the new benchmark case. Verify every agent, display and terminal test passes unchanged (layering order is the same), and the unsorted-props clean frame lands near the sorted `hot-stream/shown-clean-frame` instead of the 51–55 ms probe.

## 3. Display per-char work (#2, #7, #11; D2, D7)

- [x] 3.1 In `display::build`, collect icon starts once and look them up with a moving cursor; compute the line background face once per line. Verify `icons_take_their_runs_colour_and_their_two_cells`, the `display.rs` icon and right-align tests, and the agent tests pass, and `terminal-copy/*/10k-lines-display` medians fall back towards #404's (204 µs colored, 52 µs plain).
- [x] 3.2 Replace `has_folds`' scan with a fold count kept by `add_property`, `remove_properties` and `set_copy_text`. Verify with a test that adding, removing and range-deleting folds updates the count as the old scan would, and that `char_motion` and fold tests pass.
- [x] 3.3 Give `table_line`, `line_indent` and `unfolded_line` an iterating form of the nearby-properties lookup that allocates nothing. Verify the existing `unfolded_line` tests (including `unfolded_line_skips_the_prompt_continuation_past_the_end`) and the table and wrap tests pass, and `hot-stream/shown-clean-frame` does not regress.

## 4. Command output appends to open calls (#3, D3)

- [x] 4.1 Store the hidden line count and the count line's range in `TermRender`. Extend `append_tool_output` to open calls past the cut: append the tail, extend faces and the hidden fold, rewrite the count line; otherwise fall back to `show_tool`. Verify `output_streams_past_the_cut_of_an_open_call`, `folded_and_open_output_agree_across_chunk_boundaries` and `an_open_command_shows_twenty_lines_and_counts_the_rest` pass.
- [x] 4.2 Add a test that streams coloured output into an open call across the cut in several chunk sizes. Verify after each frame that the text and every property equal those of re-rendering the same call from scratch, and `hot-stream/tool-output-shown-open-*` drops close to the folded cases (about 0.5 ms).

## 5. Streamed messages re-render their tail (#5, D5)

- [x] 5.1 Make `markdown::render` also return the stable point (source byte, rendered char) of the last top-level block, sections included; keep its other callers unchanged. Verify with unit tests for paragraphs, headings and sections, lists, fences and tables (rendered offsets past a drawn table included).
- [x] 5.2 Store the stable point in `Stream` and make `render_stream`, `trim_message` and `end_stream` rewrite only the tail, shifting its properties, with a debug-build comparison against a full render. Verify the existing streaming tests (`a_streamed_block_renders_when_the_frame_is_built`, `hidden_streams_keep_pending_formatting_until_shown`, `tables_are_drawn_into_the_text`, `a_message_time_goes_under_its_table_rows` and the rest of `agents::tests`) pass.
- [x] 5.3 Add an equivalence test: a message with a heading, a list growing across chunks, a fence, a table and a setext underline, streamed in every split at a set of chunk sizes. Verify after each frame that the text and properties equal a single full render, and `hot-stream/shown-dirty-frame` falls well below the 44 ms of the 2026-11-08 run.

## 6. Smaller cleanups (#4, #8, #9, #12; D4, D7)

- [x] 6.1 Move the workspace list's mode line and selected-row background from `display.rs` into `workspace_list.rs`, called from `frame.rs::window_display`. Verify `display.rs` no longer mentions `workspace-list-mode` and the `workspace_list` and `frame` tests (mode line text, `1/2` position, selected background in both themes) pass.
- [x] 6.2 Add `Core::session_buffer` and replace the repeated session and buffer lookups and `expect("found above")` re-lookups in `agents.rs`. Verify `grep -c 'found above' crates/aiditor-core/src/agents.rs` is 0 and `agents::tests` pass.
- [x] 6.3 Stop cloning `Theme` in `element.rs`'s `prepaint` and `paint`. Verify `grep -n 'theme.clone()' crates/aiditor/src/element.rs` finds nothing and the `aiditor` GPUI tests (`syntax_styles_reach_font_runs_after_settings_reload`, `icons_take_their_runs_colour_and_their_two_cells`, `boxed_runs_keep_their_cells_and_share_an_outline`) pass.
- [x] 6.4 Move `echo_area`'s candidate row into `completion_row`. Verify the completion, grep header and find-file GPUI tests in `editor.rs` pass with their debug selectors unchanged.

## 7. Block ends and one block lookup (#6, D6)

- [x] 7.1 Give messages, tools, asks, answered questions, thoughts, the plan and the working line a `Stay` end marker. Replace each `len` use with the marker and move it with `set_marker` at the growth sites (`output`, `show_user`, `append_tool_output`, `show_tool` rewrites, `retime`, `trim_message`, `redraw_question`). Verify `grep -nE '\.len [+-]=' crates/aiditor-core/src/agents.rs crates/aiditor-core/src/agents/` finds no block-length arithmetic, and `agents::tests` pass.
- [x] 7.2 Add tests per block kind that check its extent after streaming, a rewrite in place, a retimed working line or waiting counter, and an added or removed meta line. Verify each block's text range is exactly its block, the meta line excluded.
- [x] 7.3 Add `Session::block_at` returning `At` and make `agent_send_input` and `agent_toggle_tool_call` dispatch on it in today's precedence; delete the per-kind `*_at_point` and `toggle_*` probes it replaces. Verify the RET and TAB tests (questions, permissions, answered rows, pastes, tool calls, thoughts, groups) pass unchanged.

## 8. Integration, measurements and records

- [x] 8.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, the `aiditor-core` dependency check from `.github/workflows/ci.yml` and `python3 -m unittest discover -s scripts -p test_benchmarks.py`. Verify all pass on macOS, and record whether Linux was run.
- [x] 8.2 Rerun the `core`, `hot_paths` and `services` suites sequentially on the 2026-11-08 host and save the TSVs next to the task 1.1 baseline. Verify every self-check passes and the README compares like-for-like cases with the baseline and the 2026-11-08 run, naming any regression.
- [x] 8.3 Update `docs/performance.md` with the new run, and correct the 2026-11-08 report: its "unchanged benchmark sources" sentence becomes a note that #457 added a table to the `hot-stream` fixture, so those comparisons are not like-for-like. Verify both documents link the new measurements.
- [x] 8.4 Record the scan in `docs/refactoring-opportunities.md` (table, cases, `Start with`, `Not checked`), and mark each row done or dropped (#10, with its reason). Verify `openspec validate fix-m6-refactor-scan-findings --strict` passes.
