# Proposal

## Why

The session buffer shows agent replies as raw text since #292: headings, emphasis, code and links read as markup, and thoughts aren't shown at all. The display model decided in #8 renders the transcript as markdown with properties while keeping the markup in the text, so search and copy still see markdown (#293).

## What Changes

- Agent messages, the echoed user prompts and agent thoughts are rendered as markdown: faces for headings, emphasis, strong, inline code and links; their markup `invisible` with no placeholder; a background line decoration on fenced and indented code blocks, whose fence lines show as blank lines; thoughts in a dim face.
- Thought chunks now appear in the transcript as their own blocks; a message chunk after a thought starts a new block, and the other way round.
- The markup stays in the rope, so swiper matches it and `M-w` or `C-w` yield markdown.
- A streamed message's text still goes in at once, but its properties are re-rendered only when the frame is built, at most once per frame, and only for that message.
- Two theme colours: `dim` (thoughts) and `code_block` (code backgrounds); headings, inline code and links take the `text.title`, `text.literal` and `text.uri` syntax colours.
- Buffers can drop the properties inside a range, and the cheap property lookup also covers nested and touching properties whose starts and ends both ascend, so a long transcript with many hidden ranges draws without scanning every property.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: replies, prompts and thoughts render as markdown; thoughts are shown; re-render once per frame.
- `text-buffers`: properties in a range can be removed.
- `settings`: the theme gains `dim` and `code_block`.

## Impact

- `crates/aiditor-core/src/agents.rs` (streaming, rendering), a new `markdown.rs` in the core, `buffer.rs` (property removal, ordering check), `display.rs` (properties near the window when folds exist), `frame.rs` (re-render hook), `settings.rs` and `settings.toml`.
- `tree-sitter-md` gets its `parser` feature; no new crates.
- `docs/editor.md` records what was built.
