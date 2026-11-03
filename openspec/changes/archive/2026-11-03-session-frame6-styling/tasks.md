# Tasks

## 1. Prompt look

- [x] 1.1 Put a `Display("❯", status-info + bold)` on the `>` of the live prompt line and of every sent and replayed echo in `agents.rs`; verify with a core test that the frame draws `❯ ` while `h.core.text()` still holds `> `
- [x] 1.2 Add a `session-user` `Background` line decoration to every line of a sent and replayed echo, not to its trailing blank line or the live prompt; verify with a display test of a two-line echo

## 2. Plan

- [x] 2.1 Rewrite `plan_line` to `  <mark> <content>` with the faces from the spec, drop the priority and the plan's rule; update the plan tests in `agents.rs` to the spec's scenarios and verify they pass

## 3. Tool-call headers and bodies

- [x] 3.1 Drop the path line and `---`/`+++` header from diff content items (`unified`, `tool_content`), keep them in `tool_output`, and drop the header branch in `diff_faces`; verify the "An edit shows its diff" scenario with a core test
- [x] 3.2 Show ` +N −M` after an edit's target, counted over its diffs, in `status-running`/`status-failed`, as frame 6's `ok`/`bad`; verify with a core test of a one-to-six-line diff
- [x] 3.3 Give a command target `session-code` in `render_tool`; verify the face with a display test of a running `ls` call
- [x] 3.4 Draw `… N more lines` in `annotation`, first checking how `display::build` faces placeholder spans (see design); verify with the long-output display test
- [x] 3.5 Add a tool-body flag to `markdown::render` that hides fence lines together with their joining newline, clip against `hidden_output`'s cut, and pass it from `render_tool`; verify open command and diff bodies have no empty fence rows, the text still holds the fences, and properties stay sorted on a 50-line output

## 4. Line indent

- [x] 4.1 Add `Decoration::Indent(cols)` and `Line::indent`; make `display::build` and `Window::row_starts` wrap an indented line at its narrower width; verify with display tests of a wrapped indented line and the cursor on one
- [x] 4.2 Draw indented lines in `element.rs`: offset text, icons, boxes, cursor and `Background` by the indent and end the background at the right text inset, leaving `Rule` and `Bar` at the edge; verify with a run against the fake agent (open a grouped edit and command)
- [x] 4.3 Set the indent on open tool-body lines in `render_tool` (status column plus two, for grouped and ungrouped calls); verify with the "An open body is indented" scenario

## 5. Session window inset

- [x] 5.1 Give session windows a 27 px left and 24 px right text inset in `frame.rs`, report each window's text start in the frame display and read it in `element.rs`; verify the "A session window" column count in a frame test and that file windows keep 12 px

## 6. Docked prompt pane

- [x] 6.1 Add `PromptPane` to `WindowDisplay` and build it in `frame.rs` per the design: pane rows from the prompt area's wrapped rows clamped to `1..=rows/3`, transcript rows above it, the transcript's top pinned to its end while point is in the prompt area, and the cursor only in the part holding point; verify the four docking scenarios with frame tests
- [x] 6.2 Draw the pane in `element.rs`: a 1 px `separator` rule across the window, 8 px padding above and below, rows anchored to the mode line, cursor in the pane; verify with a fake-agent run that the prompt sits above the mode line in a short transcript
- [x] 6.3 Check that `C-v`, `M-v`, `C-l` and `M-<` page and recenter the transcript rows with point in the transcript; verify with a frame test for `C-v`

## 7. Docs and check

- [x] 7.1 Update `docs/editor.md`'s session buffer sections (plan, tool-call blocks, the prompt and its docking, the session inset) to match the specs, and note in the frame 6 notes that the permission block is a separate task; verify `cargo test` and `cargo clippy` pass on the workspace
- [x] 7.2 Run `/design-check` on frame 6 with the fake-agent setup; verify that only the permission block and setup differences remain
