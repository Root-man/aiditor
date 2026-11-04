# Design

## Context

- Stamps come from `Core::clock: Option<String>`, the `HH:MM` text the frontend sends as `Msg::Clock` at startup and on each minute boundary (`aiditor-services/src/clock.rs`). `put_time` writes the text after an `AlignRight` space; `settle_turn` does the same for `Worked`. The first send happens mid-minute, so the text alone can't give seconds.
- Line decorations are `Decoration::{Background, Rule, Bar, Indent}` in `buffer.rs`, carried per display line and painted in `element.rs` before the text. `Rule` and `Bar` are vertical, beside the line.
- The session mode line (`agents.rs`, `Session::mode_line`) builds segments as `Vec<(String, FaceName)>`, so one segment can hold several faces. Inactive windows already repaint every span in `mode-line-inactive`.
- The echo's band is a `Prop::Line(Background(SessionUser))` on one char per line (`agents/render.rs`).
- The bundled theme is `themes.default` only; Gruvbox tables don't exist yet.

## Goals / Non-Goals

**Goals:** frame 6's turn look and times, and frame 6g's mode line colours, with the fewest new concepts.

**Non-Goals:** Gruvbox theme values, redrawing frames 9, 10 and 12, colouring the running `Working for` line, a setting to turn any of this off.

## Decisions

1. **The clock is a time of day, and the core counts on from it.** `Msg::Clock(Duration)` carries the time since local midnight; `clock::read` returns it beside the `HH:MM` text and delay it already computes. The core keeps `(since_midnight, Instant)` and formats `HH:MM:SS` from `since_midnight + (now - then)`, modulo 24 h. Sending every second instead would wake the core 60 times as often for nothing; sending `HH:MM:SS` text once a minute would need the same counting anyway. `Core::clock()` returns the formatted stamp. Local-time jumps (DST, travel) show from the next minute's message, as the workspace clock does today.

2. **Two new decorations, not a drawn-text rule.** `Decoration::Border { top: bool, face }` and `Decoration::HRule(face)`. The rule is painted by the frontend from the shaped row: it knows the x of every glyph run, so it fills the gaps, one cell clear. Writing `─` characters into the rope would put them in search, copy and kill and would need re-flowing on every resize. The `Worked` line gets `Indent(2)` for its lead-in, so the leftmost stretch is the two columns before the label. The `Working for` line gets none of this until `settle_turn`.

3. **`session-user` carries the prompt's text face.** Rather than a new `session-prompt` face, `session-user` gains `fg` and `bold`, and the echo's text gets it as a `Prop::Named` under the markdown faces, so chips, links and code win. Its `bg` matches the band, so painting it per run changes nothing. The borders use a new `session-user-rule` face, since `separator` is the darker rule colour frame 6 uses for the turn rule.

4. **Quality from the family word and the effort value.** The model's display name and value are split on non-alphanumerics and matched whole, case-insensitively, against `fable astra / opus sol / sonnet terra / haiku luna`, so `Opus 4.8`, `5.6 Sol` and `gpt-6-astra` all match, and `Solo` would not. Effort matches its value, since names are `Xhigh` or `High` while values are stable. Unknown names stay `mode-line-dim`, so a new model shows uncoloured, not wrongly coloured. This is a table in `agents.rs`, not a setting: the user asked for a fixed mapping, and the faces are already overridable.

5. **ctx reuses the status faces.** `status-running`, `status-waiting` and `status-failed` are green, yellow and red in every theme already, so no new faces.

## Risks / Trade-offs

- [A new model family (say a fifth Claude tier) shows dim] → One table row to add; the mapping is named in `docs/editor.md`.
- [The 1 px borders sit inside the row, so the first prompt line's top border can touch the descenders of the line above] → The blank line before every block keeps them apart.
- [The frontend's glyph gaps include the `AlignRight` padding, which is a space run] → The rule is painted where no glyph is drawn, so padding spaces count as empty; the label's own spaces between words keep their rule out because the gap must be wider than two cells to get a line.
- [Tests that send `Msg::Clock("14:01")`] → A harness helper `clock("14:01:05")` parses into the `Duration`.

## Migration Plan

None: `Msg::Clock` is internal to the workspace, and transcripts saved by agents don't store times.
