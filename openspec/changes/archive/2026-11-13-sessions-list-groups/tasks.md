## 1. Core

- [x] 1.1 Add `Core::wall_clock` (default `Timestamp::now`) and a `Group` enum with name, order and the by-date classification (today, yesterday, this week from Monday, older)
- [x] 1.2 Rework `SessionList`: `rows: Vec<(usize, Row)>`, `listed` keys and states, `folded` (Archived by default), `at: Option<Row>`, project name and counts
- [x] 1.3 Rewrite `render_sessions`: no header, status lines, blank line, headings with counts and fold marks, padded rows with times by age, point kept as specified, top at 0 on the first heading or row
- [x] 1.4 `sessions_move` over headings and rows; `sessions_visit` refuses headings; new `sessions_toggle` (`agent-sessions-toggle`, `TAB`) in `builtins.rs` and `keymap.toml`
- [x] 1.5 Mode line (`SessionList::decorate`) and echo line `HINTS`, wired in `frame.rs`

## 2. Tests

- [x] 2.1 Update the existing sessions tests to the new layout
- [x] 2.2 Day and week boundaries (midnight, Monday start, on a Monday), long title cut, headings counts
- [x] 2.3 TAB folds and unfolds; Archived default; `g` resets; point stays on its session and heading across answers and folds; `n`/`p` over headings
- [x] 2.4 Mode line counts and position; echo line keys

## 3. Docs

- [x] 3.1 Record what #482 built in `docs/editor.md`'s rework paragraph and the #323 note about the header
