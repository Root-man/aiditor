# Tasks

## 1. Groups in the rope

- [x] 1.1 Add `Group`, `Row`, the session's `run`, `group`, `groups`, and `Tool.ended`; end runs at messages, prompts, lines and a plan's first show, and test where groups split
- [x] 1.2 Form a group at a run's second call, joining rows and inserting its line, and add later thoughts, calls and asks as rows; test folded text and rows on consecutive lines
- [x] 1.3 Add the group line's text and props in `render.rs` and the fold runs; test verbs, tense, icon, `failed` and duration

## 2. Rows and keys

- [x] 2.1 Indent rows in `show_tool` and `render_tool`, and show thought rows; test thought rows live and replayed
- [x] 2.2 Extend `agent_toggle_tool_call` and `tool_at_point`; test each TAB scenario and `RET` from hidden text
- [x] 2.3 Keep waiting rows and pending blocks visible under a fold, and keep answered lines as rows; test waiting and answered groups

## 3. Finish

- [x] 3.1 Test motion, search, copy and replay across folded groups
- [x] 3.2 Update `docs/editor.md` with what was built, and pass fmt, clippy and tests
