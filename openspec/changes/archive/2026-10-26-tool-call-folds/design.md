## Context

`agents.rs` puts message, thought and turn-end blocks above the prompt line with `Buffer::insert_output` (past read-only, without undo, shifting the prompt area's undo history) and renders a block's markdown into properties with `markdown::render`. Other updates are kept in `Session::updates` and not shown. The display already draws `Prop::Invisible` with a placeholder as a fold and `Decoration::Rule` as a 2px bar at a line's left. `properties_near` bisects only while every property's start and end ascend in the stored order; a property added out of order switches it to a linear scan for good.

## Goals / Non-Goals

**Goals:** tool calls shown and updated in place with full input and output behind a fold; properties staying ordered as blocks change mid-transcript; folded calls cheap to move and scroll past.

**Non-Goals:** rendering diffs or terminals richly (after the MVP per the spec), permission prompts, the plan, `locations`, revealing a fold that search or `C-f` lands in, folding by mouse.

## Decisions

- **Tool state per session.** `Session::tools` maps the ACP tool call id to `Tool { call: ToolCall, start: MarkerId, header: usize, len: usize, open: bool }`: the merged call (`ToolCall::update` merges fields, `ToolCall::try_from` builds one from an update with a title), a `Stay` marker at the block's start, the header's and block's lengths in chars. Only the code that writes the block changes its text, so lengths beat an end marker that inserts at its edge would move.
- **Rewrite with a line diff.** A changed call's new block text is diffed against the old with `buffer::diff` and only the changed lines are replaced, through a new `Buffer::replace_output(range, text)` that generalises `insert_output` (same read-only and prompt bypass, history shifted by the length change). Point or the mark in an unchanged line stays put; one in a changed line goes to the hunk's start.
- **Properties without nesting.** Folded, the block's properties are the header's markdown, the rule, the dim suffix and one `Invisible(" …")` from the header's newline to the block's end; open, the content's markdown and a one-char rule at each content line's start instead of the fold. No property contains another's end, so starts and ends ascend. Toggling re-renders the block.
- **Ordered inserts.** `add_property` puts a property at its sorted place when the list is sorted and its neighbours' ends still ascend around it, instead of pushing it last. A rewritten block mid-transcript then keeps `properties_near` bisecting. Otherwise it pushes and marks the list unsorted, as now.
- **Rule on the first char.** The header's rule spans one char so it touches only the header line and sorts before the header's markdown; a rule over the whole line would contain the faces inside it.
- **Output once.** `rawOutput` shows only when there is no content item: Claude's adapter sends a result as content or as `rawOutput`, not both, and the full text either way.
- **Folds in motion.** `Buffer::unfolded_line(line, down)` maps a line starting strictly inside an invisible range to the line after that range (down) or its first line (up); `next_line` and `Window::scroll` stepping up use it, so a fold counts as no rows. This is Emacs's `line-move-ignore-invisible`.
- **`TAB` is mode-local.** `agent-toggle-tool-call` is bound in `agent-session-mode`; `TAB` stays undefined elsewhere.

## Risks / Trade-offs

- [A large output is re-diffed and re-rendered on each update] → updates per call are few; streaming terminal output deltas aren't handled (the adapter sends them only to clients that ask).
- [Point inside a fold] → only `C-n`/`C-p` skip folds; `C-f`, search or a jump can still land inside one, where the cursor shows at the placeholder. Revealing on entry comes with more fold users.
- [A markdown face crossing a content line start] → the per-line rule then sits inside it and the property list falls back to unsorted scanning, still correct.
