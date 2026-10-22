# Design

## Context

The grep service (#73) takes `Effect::Grep(GrepOp::Search { id, roots, pattern, limit })` and answers with `Msg::Grepped { search, hits, done }` batches; `GrepOp::Cancel` stops it. The core drops `Msg::Grepped` today and the harness ignores `Effect::Grep`.

The minibuffer already has what swiper needed: a spec `filter` makes a prompt list a command's own candidates (`Filtered`) instead of nucleo's matches, `preview` shows something for the selected candidate, and `Core::handle` runs `poll_completion` and `preview` after every message, with `now` and the effects list. `project-find-file` shows the pattern for a command whose candidates arrive later: it asks its own question with `read_string` and `Core::indexed` pushes the late files with `add_candidates("project-find-file", ...)`. `find_file` reads a file no buffer visits through `FileOp::Open`, so the buffer only exists once `Msg::Opened` arrives.

## Goals / Non-Goals

**Goals:** the command as the spec describes it, reusing the filtering prompt, the preview machinery and the core timers; no new `Msg`.

**Non-Goals:** generic "async candidates" support for other commands; keeping a previewed buffer's own remembered point untouched (see Risks).

## Decisions

- **A filtering prompt with an all-pass filter.** `project-grep`'s spec sets `filter` to one that keeps every candidate, so the list shows the hits as the service found them, `RET` with nothing listed already fails with `Match required`, and the typed text joins the prompt's history however it closes, as swiper's does. The handler asks its own question (`read_string(command, args, "Grep in <project>: ", None)`), since the prompt names the project; passing no choices keeps it a filtering prompt with history and preview, as a spec-declared prompt would be.
- **The core owns the search state** in a `ProjectGrep` field: the search number (a counter that only grows, as `index_walks`, so a late answer from any earlier prompt is dropped), the text last seen and its deadline, the text last searched, whether the current search has answered yet, whether it is running, its debounce timer, the root names, and the hits (`Vec<(String, Hit)>`, label and hit). Polling it from `poll_completion` (before that function's early return) means it runs after every message in the app and the harness alike:
  - prompt not open (closed by `RET`, `C-g` or anything else) while running → `GrepOp::Cancel`; no close hook needed.
  - text changed → remember it, deadline = now + 200 ms, restart a one-shot timer delivering `Msg::Tick` at the deadline (as `repoll` does).
  - text unchanged, not yet searched and deadline passed → under 3 chars: cancel and empty the list; otherwise bump the number and send `GrepOp::Search` with the project's roots, the text and limit 10,000.
  Alternative, a `Msg::GrepDebounce`: a new message for what a deadline check already does.
- **Taking answers.** `Msg::Grepped` for another number is dropped. The first answer of a search replaces the hits and the list (selection back to the first), later ones extend both; `Err` echoes the error and empties the list; `done` clears running. A minibuffer helper sets a filtering prompt's items (replace or extend, all matched) for the command named, like `add_candidates`.
- **Candidate to hit by label.** Preview and the handler get only the candidate string, so they look it up in the kept hits by label. A label can't be parsed back reliably (paths may hold `:`), and the column is not in it. The hits are kept after the prompt closes, until the next `project-grep`, so the handler finds them; labels are unique (root, path and line). A linear scan over at most 10,000 strings per selection move is cheap.
- **`Preview::Point` becomes a position in a buffer the preview resolves.** A new `Preview::FileAt(path, line, col)` resolves `path` like `Preview::File` (a visiting buffer, else the peek buffer read with `FileOp::Preview`), then turns line and column into a char offset. The existing `preview_point(p)` becomes `preview_at(buffer, p)`: a stand-in already showing that buffer keeps its scroll; otherwise the window is replaced by a copy (same buffer as the origin) or a new window on the buffer, then jumps. Swiper's `Preview::Point(p)` calls it with the origin's buffer, so its behaviour is unchanged.
- **Jump after the read.** The handler resolves the hit's path; when a buffer visits it, it switches and jumps at once. Otherwise it sends `FileOp::Open` through `find_file` and keeps `(path, line, col)` as a pending jump, which the `Msg::Opened` handler applies after `visit` when the path matches. Jumping uses `Window::jump`, which centers only when out of view. A position past the buffer's end (the file changed since the search) clamps.
- **Root names** come from the existing `root_names`, changed to take root paths, so labels match `project-find-file`'s.
- **Harness search.** `Effect::Grep(Search)` walks each root with the harness's `walk_files` (sorted), matches each line with the `regex` crate (case-insensitive when the pattern has no upper case), and sends one `Msg::Grepped` with every hit up to the limit and `done`; an invalid pattern sends its error. `Cancel` is recorded, so a test can check it. The service's own behaviour is covered by its tests.

## Risks / Trade-offs

- [Previewing a hit in a buffer that is open elsewhere leaves that buffer's remembered point at the hit, since the stand-in window writes its point back when released] → the visible windows are unaffected; a buffer reopened later starts at the hit. Accepted with a `shortcut:` comment.
- [A buffer with unsaved edits is previewed and jumped to at the disk's line numbers] → the jump clamps; the same holds for consult.
- [Large hit lists are copied into the minibuffer's items as well as kept in the core] → bounded by the 10,000 limit.
