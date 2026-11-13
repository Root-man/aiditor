## Context

`*Sessions*` (`crates/aiditor-core/src/sessions.rs`) renders plain text from each agent's `session/list` answer: a header, then one `agent  YYYY-MM-DD HH:MM  [state] title` row per session, newest first. `SessionList` keeps the rows' lines and sessions, their states (to rebuild when one changes) and the session point was last on. The workspace list (`workspace_list.rs`) already has the frame-13 idiom this change copies: key hints in the echo line (`HINTS` + `Core::key_hints`, picked in `frame.rs`) and its own mode line set in `frame.rs`'s window builder.

## Goals / Non-Goals

**Goals:** frame 14's layout as text: groups by activity, `TAB` folds, times by age, headings in `n`/`p`, point kept across rebuilds and folds, the mode line and the echo line keys.

**Non-Goals:** faces, agent glyphs and the state dots (the look task); pins, delete, archive (their tasks; Pinned and Archived stay empty); fitting columns to the window width; regrouping at midnight without a rebuild.

## Decisions

- **Plain text, padded columns.** Rows are `  <title>  <agent>  <time>  <state>` with the title padded to the longest (capped at 60, cut with `…`) and the time right-aligned in 10. Swiper and the tests keep reading the buffer text; display-time fitting as the workspace list does is left to the look task if the frame needs it. Titles start under the heading's name (two columns in).
- **A wall clock in the core.** `Msg::Clock` carries only the time of day, so grouping needs a date. `Core` gets `wall_clock: fn() -> Timestamp`, `Timestamp::now` by default, next to `time_zone`; tests set a non-capturing closure. Groups are computed at each rebuild (answers, state changes, folds, `g`), so a list left open past midnight keeps yesterday's grouping until one (`shortcut:` comment).
- **Lines as one list.** `SessionList.rows` becomes `Vec<(usize, Row)>` with `Row::Heading(Group)` and `Row::Session(agent, SessionId)`, in line order; `n`/`p`, `RET` and `TAB` read it. Every listed session's key and state is kept separately (`listed`), so `sync_session_lists` notices state changes in folded groups and the mode line counts sessions and open ones without the core.
- **Point.** `at: Option<Row>` replaces the agent-and-session pair. On a rebuild: the same session row if shown, else its group's heading if folded, else the first session row; a heading stays a heading while shown. The first heading or first session row puts windows' top at 0, as the header rule did.
- **Folds.** `folded: Vec<Group>`, `[Archived]` when the `SessionList` is made; `agent-sessions` (and so `g`) makes a new one, which is the reset.
- **Mode line.** A `SessionList::decorate` beside the workspace list's, called from `frame.rs`, sets the mode line from the buffer name, the project's name (stored at render), the counts and the position; the echo line uses a `HINTS` list through `key_hints`.

## Risks / Trade-offs

- Folds and grouping are computed in the core per rebuild; a list with hundreds of sessions rebuilds its whole text each state change, as it already did.
- With Pinned and Archived empty, "Archived starts folded" is only seen in a unit test of the defaults until #481.
