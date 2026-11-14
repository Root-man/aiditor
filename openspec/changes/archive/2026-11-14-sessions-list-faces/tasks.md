## 1. Faces

- [x] 1.1 Add `SessionListGroup`, `SessionListPinned`, `SessionListCount`, `SessionListToday`, `AgentClaude`, `AgentCodex` to `FaceName` and the bundled `settings.toml` with Gruvbox dark colours

## 2. States

- [x] 2.1 `Core::session_state` returns the state's words and face (`● exited`, `○ loading`, `○ starting`, `Activity`'s, `○ idle`); update `sessions_visit`'s loading check and callers' tests

## 3. Rendering

- [x] 3.1 Rows: four spaces, title, an `AlignRight` space, glyph and agent, time, state right-aligned in the widest state's width; headings `▾ <name>  <count>`
- [x] 3.2 Faces as `Prop::Named` per piece (marker, name, count, agent, time, state), cleared before each rebuild
- [x] 3.3 Point on column 2 of a heading and 4 of a row, in rebuilds and `n`/`p`
- [x] 3.4 `decorate`: the selected row's `completion-selected` background and `<m> open` in `status-running`

## 4. Tests and docs

- [x] 4.1 Update the sessions tests to the new text; add tests for each piece's face, live state faces, the selected row background, the right-edge alignment and the mode line's face
- [x] 4.2 `docs/editor.md`: new faces in the faces table, and the rework paragraph records what #483 built
