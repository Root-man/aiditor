## Context

`render_sessions` (sessions.rs) builds the list as plain lines and `set_text`s them; `SessionList::decorate` builds the mode line. The workspace list already draws faces with `Prop::Named` over its text, sits its state column at the window's right with a `Prop::AlignRight` space, and gives the selected row a `Decoration::Background(CompletionSelected)` in `decorate`. `Core::session_state` returns bracketed words; `Activity` (agents.rs) holds the workspace list's words and faces for live sessions.

## Goals / Non-Goals

**Goals:** frame 14's faces, words and column placement; core tests per piece's face.

**Non-Goals:** the workspace list's narrow-window `fit` (abbreviating states, dropping columns): a narrow window falls back to one space after the title and cuts the line. Pins, archive and delete keys. A glyph or face for agents other than `claude` and `codex`.

## Decisions

- **Faces as properties on the text**, as the workspace list does, collected while lines are built (`(range, FaceRef list)` by char offset) and added after `set_text`, with `remove_properties` first. Display-time spans (`fit`) would be the only other way and are only needed for narrow layouts.
- **Right-hand columns with one `AlignRight` space after the title.** Frame 14's grid has the title column flexible and the rest fixed at the right. One property gives that; titles are no longer padded, which also removes the wide title column the #482 notes mentioned. The state column is padded to the widest listed state (right-aligned) so times line up whether or not a row has a state.
- **`session_state` returns `(&'static str, FaceName)`.** Ended, loading and starting first (the core's own states), then `Session::activity()` for the workspace list's words and faces, else `○ idle`. Using `activity()` means a blocking question reads `● awaiting answer`, as on the workspace list, where `[waiting]` covered both. Loading and starting read `○ loading` / `○ starting` in `mode-line-dim`, as idle: asked and settled with the user.
- **Six new named faces** (`session-list-group`, `-pinned`, `-count`, `-today`, `agent-claude`, `agent-codex`) rather than reusing faces of the same Gruvbox colour, since buffers store face names and a theme must be able to tell them apart (#345). Bundled values are Gruvbox dark's, as the `quality-*` faces did, so the user's Gruvbox theme needs no new entries. Archived is `annotation` + bold and other times `mode-line-dim` (frame 14's `#a89984`), so no face for them.
- **Point's column**: 2 on a heading, 4 on a row, from one helper used by `render_sessions` and `sessions_move`.
- **Selected row**: `decorate` pushes `Background(CompletionSelected)` on the cursor's display lines when the cursor's buffer line is a row, as `Snapshot::decorate`.
- **Mode line**: `summary` becomes a `Segment` so `<m> open` carries `status-running`.

## Risks / Trade-offs

- [Narrow windows] lines that don't fit get one space after the title and lose the right-hand alignment → acceptable until a narrow frame for the list exists; noted in docs.
- [Text change] tests matching `[idle]` etc. change wholesale → updated in this change.
