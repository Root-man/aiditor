# Design

## Context

The buffer already holds faces, `invisible` ranges and line decorations, and the display draws them (`display.rs`). Nothing produces them for session buffers yet: `agents.rs` inserts message chunks as plain text with `insert_output` and keeps a `streaming` flag. With any invisible range the display currently collects every property of the buffer and scans them per char, and `properties_near` has a fast path only for disjoint, ordered properties. Markdown makes hidden ranges common, so both paths matter for long sessions.

## Goals / Non-Goals

**Goals:**
- Render messages, thoughts and echoed prompts from their text alone, in the core, testable through the harness.
- Keep drawing a long transcript cheap: properties looked up near the window, not over the whole buffer.

**Non-Goals:**
- Lists, block quotes, tables, images, reference links, HTML and an ATX heading's closing `#`s: shown as their text.
- Syntax highlighting inside code blocks, proportional fonts, larger headings.
- Re-rendering on a theme change; colours are taken when a block renders.
- Folds counting in wrap, scroll and point's column (the existing `shortcut:` notes stay).

## Decisions

- **Parser: `tree-sitter-md`'s `MarkdownParser`.** The crate is already a dependency for `.md` highlighting; its `parser` feature adds only `tree-sitter`, already used. It parses the block grammar and each inline node with the inline grammar. pulldown-cmark would be a new dependency for the same result.
- **A pure function `markdown::render(text, thought, theme) -> Vec<(Range<usize>, Prop)>`** in char offsets of `text`. It collects faces, hidden ranges and code lines from the trees, cuts the text at every boundary, merges the faces of each piece, drops faces under hidden text, joins equal neighbours, and returns the props sorted by start then end. Faces and hidden ranges are then disjoint, and code-line decorations cover whole lines (newline included), so starts and ends both ascend in the common cases.
- **Properties sorted, not disjoint, for the fast path.** `properties_near` binary-searches on ends and on starts; that is correct whenever both ascend, which edits keep (markers move monotonically). `add_property` keeps the fast path while the new property's start and end are at or past the last one's. Removing properties keeps order.
- **`Buffer::remove_properties(range)`** drops properties lying within `range` with their markers, so a block re-renders without leaving stale ranges.
- **Re-render in `Core::frame()`.** The frontend builds a frame once per render, so the session's streamed block, marked dirty by each chunk, re-renders there: parse `marker..block end`, remove its properties, add the new ones. A block that stops streaming (a new block starts) renders at once if still dirty. The echoed prompt renders once on send. The text still goes in per chunk, so search and point see it at once.
- **Folds draw near the window.** With invisible ranges, `display::build` asks `properties_near(first..last)` and widens `last` by the number of newlines hidden in the range until it stops growing, instead of collecting every property.
- **Thoughts are blocks.** `Session::streaming: bool` becomes the streamed block's start marker and kind (message or thought); a chunk of the other kind starts a new block.
- **Theme:** `dim` (0x808080) and `code_block` (0x2a2a2a) join `Theme`; heading, inline code and link colours come from `theme.syntax` (`text.title`, `text.literal`, `text.uri`, falling back to the foreground).

## Risks / Trade-offs

- [Every edit moves every marker; each rendered property adds two.] → Fine for hundreds of messages; note a `shortcut:` to move to an interval tree when typing lags in long sessions.
- [Unusual nesting breaks ascending order] → `properties_near` falls back to the linear scan; correct, just slower.
- [Re-parsing a long streaming message each frame] → One block only, tree-sitter parses a few KB in well under a millisecond.
- [Wrap counts hidden chars] → Rows with hidden markup break a little early; existing `shortcut:` notes already cover it.
