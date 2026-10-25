# Proposal

## Why

Code buffers are drawn in one colour. JS/TS is the first language of the M5 tree-sitter work ([#256](https://github.com/Root-man/aiditor/issues/256)), and it builds the pipeline the other languages reuse.

## What Changes

- Visiting a `.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`, `.mts`, `.cts` or `.tsx` file parses it with tree-sitter (JavaScript, TypeScript or TSX grammar).
- Every text change (typing, undo, redo, revert, reload) is applied to the tree and the tree is reparsed incrementally on the core thread before the next display (a `shortcut:` until parsing moves to the syntax service).
- The display builder runs the grammar's highlights query over the visible lines only and colours text by capture name from a new `theme.syntax` palette; highlights are not text properties. JavaScript uses its JS and JSX queries; TypeScript and TSX combine JavaScript's with their own.
- `theme.syntax` is a table of capture name to colour in the bundled settings; `settings.toml` overrides single entries.

## Capabilities

### New Capabilities
- `syntax-highlighting`: picking a language for a visited file, keeping its syntax tree current, and colouring the visible text from the theme's syntax palette.

### Modified Capabilities

## Impact

- `crates/aiditor-core/Cargo.toml`: `tree-sitter`, `tree-sitter-javascript`, `tree-sitter-typescript`, `streaming-iterator` if not re-exported.
- New `crates/aiditor-core/src/syntax.rs`; `buffer.rs` (a syntax field updated by raw edits), `display.rs` (syntax faces), `frame.rs`/`minibuffer.rs` (reparse before building), `settings.rs` and `settings.toml` (`theme.syntax`).
- `docs/editor.md`: Language support paragraph.
