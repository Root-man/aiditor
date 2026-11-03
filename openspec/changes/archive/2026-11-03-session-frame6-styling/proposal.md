# Proposal

## Why

A design check of the session buffer against frame 6 of the visual direction canvas found styling gaps (#441): faces the theme already defines for the session go unused, edit bodies show paths and headers the frame leaves out, tool bodies carry empty fence rows and run full width, and the prompt sits inline instead of docked at the window's bottom. M6's gate exercises this buffer by hand, so it should look as approved.

## What Changes

- Sent and replayed prompts draw their lines on the `session-user` background, and the prompt glyph `> ` is drawn as `❯ ` in `status-info`, bold, in the echo and the live prompt. The rope keeps `> `, so copying and tests see the same text.
- **Prompt docking.** A session window draws its prompt area as a pane pinned to the window's bottom, under a 1px `separator` rule with 8px above and below, and the transcript in the rows above it. With point in the prompt area the transcript shows its last lines; with point in the transcript it scrolls as any window does. The prompt pane grows with its text up to a third of the window's rows, then scrolls to keep point.
- Session windows inset their text 27px from the left (the frame's 24px padding plus the 3px message bar) and 24px from the right.
- An edit's open body shows only its diff's hunks, starting at `@@`: no path line and no `---`/`+++` header.
- Fence lines in tool-call bodies take no row: the fence and its newline are hidden. Fences stay in the rope.
- Open tool-call bodies are indented under their header and their `session-block` background stops short of the window's right inset, as frame 6 draws them.
- An edit's header shows `+N −M` after its path, in the status marks' `status-running` and `status-failed` faces, counted over its diffs.
- A command's target in its header is in `session-code`.
- `… N more lines` is drawn in `annotation`.
- **BREAKING (display):** plan entries read `<mark> <content>`: `✓` completed and `○` pending, both in `session-thought`, and `●` in `status-waiting` with the content plain for in-progress. The priority is no longer shown, and the rule beside the plan goes.

Out of scope: the permission block's `key-hint` number keys and `permission` background, which become their own task.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: the echo's background and glyph, the docked prompt pane, the plan's marks, an edit body showing only hunks, hidden fence rows, indented tool bodies, `+N −M` on edit headers, command targets in `session-code`, the dim more-lines count.
- `display-model`: session windows' wider text inset; a line indent decoration that shifts a line and its background; a window display split into a transcript and a pinned prompt pane.
- `frontend-rendering`: drawing the prompt pane with its rule and padding, and indented line backgrounds.

## Impact

- `crates/aiditor-core/src/agents.rs` (echo and prompt properties, plan block), `agents/render.rs` (diff, fences, headers, more-lines face, body indent), `markdown.rs` (fence hiding for tool bodies), `frame.rs` and `display.rs` (inset, indent, prompt pane), `buffer.rs` (`Decoration::Indent`).
- `crates/aiditor/src/element.rs`: draws the prompt pane and indented backgrounds.
- Display and core tests that assert plan lines, diff bodies or open tool bodies change.
- `docs/editor.md`: the session buffer sections and the frame 6 notes.
