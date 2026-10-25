# Design

## Context

`syntax.rs` maps an extension to a `Grammar` (language plus compiled highlights query) in a `LazyLock`; everything after that is language-independent.

## Goals / Non-Goals

**Goals:** Markdown's block structure highlighted with `tree-sitter-md`'s own block query.

**Non-Goals:** the inline grammar (emphasis, code spans, links), code inside fences in its own language, bold or italic headings. All need injections or face attributes.

## Decisions

- **`tree-sitter-md` 0.5** from crates.io: `LANGUAGE` (block grammar) and `HIGHLIGHT_QUERY_BLOCK`, on `tree-sitter` 0.27 like the other grammars. The block grammar alone is one tree, so it fits the existing one-grammar-per-buffer `Syntax`.
- **Extensions** `.md` and `.markdown`.
- **Palette.** The block query's captures are nvim's older names: `text.title` (headings, keyword colour as VS Code's `markup.heading`), `text.literal` (code blocks, string colour as `markup.inline.raw`), `text.uri` and `text.reference` (link reference definitions), `punctuation.special` (heading, list, quote markers, thematic breaks; VS Code's `markup.list` colour). `punctuation.delimiter` and `string.escape` are left to the foreground and `string` as before.
- **Fence content.** The query captures `code_fence_content` as `@none`; with no `none` entry the capture is skipped, so the content keeps the enclosing `fenced_code_block`'s `text.literal` colour, as Emacs's `markdown-mode` draws code blocks in one face. Injections will colour it by language later.

## Risks / Trade-offs

- [Paragraph text inside headings is one `inline` node] → headings colour whole, including inline markup; acceptable until the inline grammar.
