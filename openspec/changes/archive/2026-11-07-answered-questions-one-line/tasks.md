# Tasks

## 1. The answered line

- [x] 1.1 `Question::content` returns the sent `(name, answer)` pairs and a `label()` (`<message>` or `<n> questions`); the summary joins answers with ` · ` for several. Verify: the questions.rs unit tests.
- [x] 1.2 Close every question outcome as `▸ <mark> <label>  <outcome>` with the clock time and faces; `○ … Closed` for a withdrawal, `Closed, the turn ended first` at a non-blocking turn's end; keep permission lines. Verify: agents.rs tests updated for the new lines, plus one checking each outcome's text and mark faces.
- [x] 1.3 Keep `Answered` entries per session and draw the sent rows folded under `Invisible`, indented in a group. Verify: test that the rows are invisible until `TAB`.

## 2. TAB and the replaced call

- [x] 2.1 `TAB` on an answered line or its rows opens and folds it; nothing sent falls through. Verify: test opening and folding, point moving to the line, and `No tool call here` on a skipped line.
- [x] 2.2 Hide the question's tool call once closed, alone and in a group, through later updates. Verify: tests with a lone call and an open group.
- [x] 2.3 A replayed question's tool call shows no question block. Verify: a load-replay test.

## 3. Docs and checks

- [x] 3.1 Update `docs/editor.md`'s Questions paragraph and `aiditor-services/tests/scenario.rs`'s `○ withdrawn` check.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.
