# Tasks

## 1. Clock to the second

- [x] 1.1 `clock::read` also returns the time since local midnight; extend `the_clock_aligns_to_minutes_and_rolls_over_at_midnight` to assert it. Verify: `cargo test -p aiditor-services clock`.
- [x] 1.2 `Msg::Clock(Duration)`; the core keeps `(since_midnight, Instant)` and `Core::clock()` formats `HH:MM:SS` counted on from it, wrapping at midnight; the frontend sends the new value at startup and each wakeup while its workspace clock keeps `HH:MM`. Add a harness helper taking `"14:01:05"`, move the `Msg::Clock` tests in `agents.rs` to it and add the "Seconds count between minute updates" and midnight scenarios. Verify: `cargo test -p aiditor-core` and the `editor.rs` clock test.

## 2. Mode line colours

- [x] 2.1 Add `quality-poor` … `quality-artifact` and `session-user-rule` to `FaceName` and `settings.toml`'s `themes.default` (frame 6g's hexes), and `fg` and bold to `session-user`. Verify: a settings test that the bundled theme defines all eight with a foreground.
- [x] 2.2 Colour the model and effort in `Session::mode_line` by the family-word and effort-value tables, ` · ` in `mode-line-dim`, unknown in `mode-line-dim`. Verify: tests for the "Quality colours", "Older models" and inactive scenarios, and the existing mode line tests still pass.
- [x] 2.3 Colour `ctx N%` with `status-running` / `status-waiting` / `status-failed` at the 50 and 80 bounds. Verify: a test over 49, 50, 79 and 80%.

## 3. Turn look

- [x] 3.1 Add `Decoration::Border { top, face }` and `Decoration::HRule(face)`, carried per display line like the others. Verify: a `display.rs` test for the "Borders and a horizontal rule" scenario.
- [x] 3.2 Paint them in `element.rs`: borders as 1 px lines at the row's top or bottom edge across the window, the rule through the row's middle in each glyph-free stretch, a cell clear of glyphs. Verify: an element test that a settled turn's row paints three rule quads and an echo's rows paint top and bottom border quads.
- [x] 3.3 Echoes, sent and replayed: top border on the first line, bottom on the last, in `session-user-rule`, and the text in `session-user` under markdown, chip and time faces. Verify: the "A sent prompt", "A one-line prompt" and "The live prompt" scenarios in `agents.rs`.
- [x] 3.4 `settle_turn`: the `Worked` label and time in `annotation`, the line `Indent(2)` and `HRule(separator)`; the running line unchanged. Verify: tests that a settled line carries the rule however the turn ends and the running line doesn't.

## 4. Docs and check

- [x] 4.1 Update `docs/editor.md`: Times (`HH:MM:SS`, the time-of-day clock), Working timer (the settled rule), the agent session mode line (quality and ctx colours, the mapping table), the prompt band, and the faces list. Verify: the sections name every new face and the thresholds.
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test`, then `/design-check` frame 6 for the turn rule, prompt band, seconds and mode line colours. Verify: all pass and the design check reports no differences beyond the Gruvbox-only colours.
