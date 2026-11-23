# Design

## Context

See proposal.md for the findings. The state these fixes build on:

- The diff view's `diff-stage`, `diff-unstage` and `diff-discard` all enter `Ctx::diff_update`. For files it pushes one `FileOp::GitUpdate`; for a change it either edits the buffer (a discard in a `Changes` diff) or pushes a `GitUpdate` with a patch built from `Shown::old`, the index text the view last read. The runner runs every `FileOp` on its own `spawn_blocking` task, so two updates run at once. `on_git_updated` echoes a failure, or bumps `DiffList::pending` and asks `GitStatus`. `took_answer` renders the answer and drops the shown diff, which asks git again. The only guard today is `shown.request != 0 || shown.timer.is_some()` ("The diff is being worked out"), which is clear between the update's success and the status answer.
- `DiffList::pending` also counts `g` refreshes. `diff_refresh` resets it to 1 when the roots changed, so a status asked for the old roots is never taken.
- `render_sessions` (`sessions.rs`) and `render_diff_list` (`diff.rs`) each open with the same point lookup, over the current layout and every other workspace's saved layout. Each ends with the same `lines: Vec<Vec<(String, Option<Prop>)>>` → `remove_properties` + `set_text` + `add_property` loop, then sets the buffer's point marker and every window's point. Only the sessions list then scrolls to the top (`set_top(0)`) when its target is the first row.
- `Windows::windows_mut()` walks the current layout and the diff view's `saved` one. There is no shared read-only walk of the current layout.
- In `display::build`, the main loop's `fold` closure and `rest` each match `Prop::Invisible`/`Prop::Display` into placeholder pieces; `fold` also handles `Prop::AlignRight`.
- CI runs `cargo clippy --all-targets -- -D warnings`, so dead code fails the build.

## Goals / Non-Goals

**Goals:**
- #1 has tests that fail before the fix (the spec's scenarios).
- #2 to #10 change no behavior: every existing test passes unchanged, and only `use` lines and paths change in tests.
- #4 moves code without editing it, so `git show --color-moved` shows it as moved.

**Non-Goals:**
- Queuing a second git update to run after the first. Refusing is enough by hand, and it is what the view already does while a diff is being worked out.
- `*Workspaces*`' `Snapshot::install`, which writes rows one at a time with `insert_output` and keeps its own `Kind`. It doesn't share the `lines` shape, so it stays as it is.
- `pub` items other than `pub fn` (types, fields, modules), and `pub fn`s in private modules, which rustc already checks for dead code.

## Decisions

### D1 · `updating` lives on `DiffList` and ends with the refreshed answer (#1)

`DiffList` gets `updating: bool`. `diff_update` returns `Err("Git is still updating")` when it is set, after the revisions check and before anything else, so both the list and the change paths are covered with one check. It is set where a `GitUpdate` is pushed (the two `self.effects.push(Effect::File(FileOp::GitUpdate { .. }))` sites). It is cleared in `on_git_updated` on failure, and in `took_answer` (every status answer the list takes).

Clearing in `took_answer`, not on `on_git_updated`'s success, keeps the wait over the window where `Shown::old` is stale. It also ends the wait when a `g` refresh with new roots overtakes the update's own status, because that refresh's answer is the one taken.

Alternative: `updating || pending > 0`. That also blocks after a plain `g`, which the spec doesn't want, and a status for old roots would never let `pending` reach 0.

The buffer-only discard is refused too: one check at the top is simpler than threading the flag past the action match. Its edit would race nothing, but the list is about to change under it.

### D2 · Three list helpers on `Core` (#2)

- `Core::list_point(id) -> usize`: point of the first window showing `id`, in the current layout or another workspace's, else the buffer's own point marker.
- `Buffer::set_lines(lines: &[Vec<(String, Option<Prop>)>])`: clears the properties, sets the text to the lines joined by `\n`, and adds each piece's property. It goes in `buffer.rs` because it only touches the buffer.
- `Core::place_list_point(id, point, top: bool)`: sets the point marker and the point of every window showing `id`, a saved layout's included, and `set_top(0)` on each when `top`.

The sessions list passes `top = first`; the diff list passes `false`. Each `render_*` computes its own target line and column between `set_lines` and `place_list_point`, because the fallbacks differ: the end of the buffer for sessions, the start for the diff list.

Alternative: one `write_list(id, lines, target)` doing all three. Its target and fallback rules differ per list and would need parameters of their own; the `top` flag is the only one kept.

### D3 · `fold_at` returns the pieces; `rest` counts them (#3)

A closure `fold_at(at) -> Option<(usize, Vec<(Cow<str>, Face)>)>` covers `Invisible` (one piece in the `Annotation` face) and `Display` (each piece's resolved face) for the property containing `at`. The main loop uses `fold_at(pos).or_else(|| align_right(pos))`. `rest` uses `fold_at(end)` and adds up the pieces' char counts. `rest` now resolves faces it doesn't use; that only happens on lines with an `AlignRight`, and only for their folds.

Folds now win over an `AlignRight` that starts inside one. Before, whichever came first in property order won. A hidden alignment space shouldn't pad, so this is the correct order; the existing align-right and fold tests must pass unchanged, and if one doesn't, keep the old order and record why.

### D4 · `git.rs` is a plain move (#4)

These move to `aiditor-services/src/git.rs`: `head`, `repository`, `git_status`, `git_update`, `check`, `porcelain_v2`, `git_change`, `git_diff`, `side_bytes`, `git_refs`, `git_diff_files`, `snapshot`, `clear_snapshots`, `name_status`, `tree_ish`, `git`, `with_objects`, `no_filter`, `changed` and `own_filter`, with their tests. `files::run` keeps its match and calls `git::…`. `root` stays in `files.rs`: it finds a project root, not a git answer. `read`, `create_temp` and `NEXT_TEMP` stay in `files.rs` and become `pub(crate)` for `git.rs`. A test moves when it calls a moved function directly; tests that go through `files::run` stay. The services bench and `main.rs` change to `git::snapshot` and `git::clear_snapshots`.

The move is its own commit, before #8, so the move diff has no edits in it.

### D5 · `Windows::iter()` yields ids too (#5)

`Windows::iter() -> impl Iterator<Item = (WindowId, &Window)>` walks the selected window, then `others`, the same order as today's `once(..).chain(others)`. `workspace_list.rs` needs the ids; the other sites ignore them. It doesn't include `saved`, unlike `windows_mut`, because every read-only site today walks the current layout only. The sites:

- `frame.rs` `display_buffer`
- `diff.rs` `on_git_diffed`'s `shown_window`
- `diff.rs` `diff_update`
- `workspace_list.rs`
- the two point lookups that D2 turns into `list_point`

`diff_file`'s `values_mut` walk stays.

### D6 · `pub(crate)` first, then let clippy decide (#6)

The 37 functions are those from the scan:

- `buffer.rs`: `create_scratch`, `recent`, `raise`, `find_here`, `is_here`, `visiting_in`, `copy_mode`, `redo`, `add_marker`, `set_marker`, `add_property`, `remove_properties`, `properties_near`, `reparse`, `remove_marker`, `line_col`, `char_idx`, `set_saved`, `set_text`
- `display.rs`: `row_starts`
- `settings.rs`: `text_rows`, `drawn_rows`
- `terminal.rs`: `feed_at`, `copy_text`
- `window.rs`: `set_point`, `set_mark`, `push_mark`, `pop_mark`, `ring`, `set_active`, `top_row`, `jump`, `set_top`, `hscroll`
- `lib.rs`: `Ctx::start_timer`, `Ctx::cancel_timer`, `Core::this_command`

Re-check the list with a grep over the other crates (their tests, benches and integration tests included) before the change. Then run `cargo clippy --all-targets -- -D warnings`:

- A function it reports dead in the non-test build that only tests call becomes `#[cfg(test)]` if it is a test helper.
- Any other dead function is deleted, with tests that test only it.
- A function that fails to compile from another crate goes back to `pub`.

### D7 · `git_output` replaces `check` (#8)

`fn git_output(git: &mut Command) -> Result<Vec<u8>, String>` runs the command and answers its stdout on success, else its trimmed stderr; a spawn failure answers `git: <err>`. It replaces `check`, `snapshot`'s `run` closure and the inline checks in `git_status`, `git_refs` and `git_diff_files`. `git_update`'s `Apply` step, which writes stdin, and `side_bytes`, which reads stderr only after a second probe, keep their own handling. Error texts stay byte for byte the same.

### D8 · `terminal_paint` stays under 8 parameters (#10)

The terminal block becomes `fn terminal_paint(terminal, origin, cell, selected, font, settings, window) -> TerminalPaint`, where `TerminalPaint { cells, cursor, cursor_char, hollow }` and `origin` is the grid's top left. `fn cursor_run(len, font, theme) -> TextRun` builds the cursor's run for both the text and the terminal cursor. If the function needs more than these seven parameters, leave the block inline, keep only `cursor_run`, and record that in the scan record.

## Risks / Trade-offs

- [#1 A status answer never arrives, so `updating` never clears] → Every `FileOp::GitStatus` answers `Msg::GitStatus`, an error per root included. If the view is quit and opened again, the new list starts with `updating` false.
- [#3 Fold-over-align order changes a drawn line] → Existing display tests are the check (D3). Keep the old order if one fails.
- [#6 A `pub fn` is used by a crate the grep missed] → The workspace build and clippy over `--all-targets` catch it; it goes back to `pub`.
- [#4 and #8 in one place make review harder] → Separate commits: the move first, the runner second.
