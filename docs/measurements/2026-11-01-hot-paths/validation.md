# Validation of fix-editor-hot-paths-and-autosave

Local execution: macOS 27.0.1, Apple M5 Pro, Rust 1.99.0. The workspace run passed 47 GPUI tests, 729 core tests, 94 service unit tests and all integration targets, including 11 filesystem-sync and both crash-recovery tests. The final core follow-up passed 730 tests with three ignored performance diagnostics. The two newly added diagnostics were also executed explicitly in release mode; their output is retained beside this file. Watcher integration tests required host access after the sandboxed run could not receive native filesystem notifications.

`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, all 21 benchmark-reporting Python tests, the core GPUI/Tokio dependency boundary, and strict OpenSpec validation passed. All 92 matched headless cases and both real-window GUI reports passed their self-checks/parsers. The supplied baseline hashes still match. Its patch artifact contains intentional context-line whitespace and is excluded from the staged whitespace check; source and documentation are checked normally. The existing CI matrix is unchanged and runs these checks on `ubuntu-latest` and `macos-latest`.

Linux and macOS CI both passed on implementation commit `05af0ac2aa07f598e7b30cfaf1eb5558afdd784d` in [CI run 41309485440](https://github.com/Root-man/aiditor/actions/runs/41309485440); [the job/step conclusions](ci-evidence.json) are retained. The Linux job includes formatting, dependency/profile boundaries, Clippy, the full test suite and Python reporting tests. Local timings remain macOS measurements.

## Auto-save scenarios

Evidence lives in [core autosave tests](../../../crates/aiditor-core/src/autosave.rs), [worker tests](../../../crates/aiditor-services/src/recovery.rs), [GUI tests](../../../crates/aiditor/src/editor.rs), and [crash recovery](../../../crates/aiditor-services/tests/crash_recovery.rs).

| Spec scenario | Evidence |
| --- | --- |
| Idle after an edit | `an_edit_is_auto_saved_after_two_idle_seconds_and_removed_on_save`; worker header/body/permissions test; crash-recovery tests verify the visited file stays unchanged. |
| Not before the delay | The same core test advances one second, edits again, and verifies the restarted two-second delay before persistence. |
| Typing without pause | `typing_without_pause_is_auto_saved_every_thirty_seconds`. |
| Scratch buffer | `scratch_messages_and_the_minibuffer_are_not_auto_saved`. |
| Messages and minibuffer | The same test retains text while creating no recovery files. |
| Edits while an auto-save runs | `delayed_persistence_acknowledges_its_snapshot_and_newer_edits_follow_without_input`. |
| Save | Idle/save test and GPUI `recovery_files_are_written_and_removed_on_quit`; cleanup checks cover already confirmed copies. |
| Kill | `killing_or_quitting_removes_recovery_files` and the pending/failed/confirmed retirement matrix. |
| Quit | The same core test, real worker FIFO cleanup and headless relaunch/save/quit coverage. |
| Cleanup during a pending write | `retired_replies_leave_clean_killed_and_new_requests_untouched`: save, explicit revert, kill and quit across pending, failed and confirmed states, followed by late success/failure replies. |
| Further edits during a regular save | `edits_during_a_save_keep_the_recovery_file` and the delayed completion/newer request tests. |
| Quit with pending recovery notifications | `saturated_or_closed_gui_receivers_do_not_block_worker_cleanup`: 300 writes exceed the actual GUI channel's 256 slots; queued deletion completes before replies are drained, also with a closed receiver. Core retirement covers late replies after quit. |
| Transient failure without another edit | `an_obstructed_auto_save_retries_without_input_and_recovers_on_relaunch`; real worker test observes a failure reply before removing the obstruction and driving the retry deadline. |
| Latest text on retry | `failure_deadlines_survive_input_and_report_changed_reasons_until_success` and `worker_failure_and_core_deadline_retry_persist_the_latest_snapshot`. |
| Repeated failure | The core failure test checks unchanged/changed reasons and resets the episode after success. `autosave_worker_failures_report_once_per_episode_and_cleanup_still_logs` runs the real worker through the GUI logging layer: three identical failures produce one diagnostic containing only the file and reason, success resets the episode, and standalone cleanup errors still reach `*Messages*`. `an_older_attempt_cannot_acknowledge_a_retry_of_the_same_tick` verifies distinct attempt identities. |
| Failed replacement of a recovered file | `a_replaced_recovery_file_goes_only_after_the_write`; multiple-copy core tests retain every applicable path through failure/pending writes. |
| Replacement succeeds but old-copy cleanup fails | `persisted_replacements_report_cleanup_errors_separately` verifies a valid replacement and surviving obstructed old path. `older_success_and_cleanup_failures_preserve_later_recovery_copies` verifies core acknowledgement and the cleanup diagnostic. |

Worker inspection confirms success follows write, `sync_all` and rename; old-copy deletion happens afterwards and has a separate result. Header fields, temp/final names, directory permissions and own-PID protection are retained. GUI callbacks spawn async sends on the existing runtime; no blocking/lossy completion send is used. Headless waits for replies or timer deadlines. Unavailable/closed service enqueue paths create correlated failure messages.

## Stream scenarios

Evidence lives in [agent tests and finalization](../../../crates/aiditor-core/src/agents.rs) and [frame construction](../../../crates/aiditor-core/src/frame.rs).

| Spec scenario | Evidence |
| --- | --- |
| Chunks wait for the frame | `a_streamed_block_renders_when_the_frame_is_built`: immediate raw text, then named formatting after the frame. |
| Earlier blocks are untouched | The same test preserves prior block properties across another reply and repeated frames. |
| Hidden streaming text catches up on display | `hidden_streams_keep_pending_formatting_until_shown`; inactive-workspace test checks retained dirty state and accumulated text. |
| An inactive pane is still shown | `inactive_panes_duplicate_panes_and_prompt_origins_flush_streams`. The shown set is derived from laid-out panes and deduplicated by buffer. |
| A prompt leaves its origin pane shown | The same test streams while `M-x` is active over the origin pane. |
| A hidden block finishes before another begins | `inactive_workspace_streams_finalize_hidden_blocks_and_kill_safely` tests a hidden message/type boundary. Inspection confirms `output`, a new tool call, a new plan, every permission request and submitted prompt still call unconditional `end_stream`; existing tool/plan/permission/prompt tests pass. Dirty tool-call flushing remains outside the shown-session condition. |
| A hidden session is killed | The inactive-workspace test kills the session and builds another frame successfully. |

The focused suite separately measures shown/hidden clean/dirty frames and first show of accumulated hidden formatting. Deferred first-show cost is reported, rather than counted as eliminated work.

## Swiper presentation scenarios

Evidence lives in [Swiper tests](../../../crates/aiditor-core/src/swiper.rs), [display overlay tests](../../../crates/aiditor-core/src/display.rs) and [GPUI completion tests](../../../crates/aiditor/src/editor.rs).

| Spec scenario | Evidence |
| --- | --- |
| A long line with a match beyond the displayed text | `chunk_spanning_lines_keep_unicode_matches_and_final_line_order` and `result_width_and_unicode_styles_change_without_changing_navigation` check clipped rows and full-source preview/acceptance. |
| Resizing a result | The width/Unicode test changes widths and cell width while keeping selection and the full-source target. Existing GPUI zoom/layout tests pass. |
| A narrow line-number prefix | The same core test includes zero and one column; the GPUI test includes a two-column budget for a three-column prefix. Canonical candidate selection remains independent of presentation. |
| Unicode at the clipping boundary | The width test clips `é🙂`, checking all style endpoints are valid byte boundaries within the displayed string; GPUI verifies run lengths and shaping at four budgets. |
| Snapshot context survives clipping | `result_syntax_retains_multiline_and_injected_context` clips at token boundaries and covers offscreen definitions. Filtering/scroll/theme and pending-edit/revert snapshot tests pass. GPUI source-face/selection tests pass; `long_swiper_rows_reach_gpui_as_bounded_text_and_valid_selected_runs` checks ten long rows at four budgets. |

Preview matching tests cover full-line anchors, ordered words, exclusions, smart case, literal fallback, and empty matches. Retained-span tests bound output independently of 1,000 versus 1,000,000 matches; clipped/folded overlay tests preserve original first-match identity. Existing project-grep preview/navigation/cleanup tests pass. The component diagnostic separates complete-line copying/exclusion scans, retained-span conversion and unchanged whole-line wrap layout.

## Syntax and structural acceptance

Fresh-parse comparisons cover Rust, JavaScript, TypeScript/TSX and Markdown injections, including same-length renames, shadowing, declaration-value ordering, edits before a scope, changed outer bindings, undo/redo/revert, injection-language changes and isolated snapshots. Existing palette and offscreen-definition tests pass. The release 10k/100k diagnostic rebuilt one scope at each size; full existing syntax-edit and revert benchmark cases also passed.

- [x] All six findings have implementation and focused validation evidence above and in the measurement report.
- [x] Explicit edits and Tree-sitter reuse govern scope-relative records; definition captures are batched, and mutable caches remain isolated across snapshots.
- [x] Preview storage/conversion is bounded to displayed intersections, retaining full-line semantics and first-match identity.
- [x] Hidden streams retain dirty state; shown panes flush before display; block finalization remains unconditional.
- [x] Persistence is acknowledged separately from requests, with timer retries, error deduplication and retirement/replacement ordering.
- [x] Worker replies do not block shutdown; all callers use the correlated protocol.
- [x] Swiper strings/runs are bounded before presentation; full-source navigation and current-theme styling remain intact.
- [x] Existing benchmark definitions/sampling are preserved except the specified assertion repair; distinct focused cases and all raw reports are retained.
- [x] The unchanged macOS/Linux CI matrix passed on implementation commit `05af0ac`. Core excludes GPUI/Tokio, and no dependency, setting or on-disk recovery format was added. The omitted grep-list and terminal-theme candidates were not changed.
