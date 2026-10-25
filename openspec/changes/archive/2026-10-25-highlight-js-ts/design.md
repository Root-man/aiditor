# Design

## Context

Every text change in a buffer goes through `Buffer::raw_insert` and `raw_delete` (insert, delete, undo, redo, `replace` for reverts, `set_text`). The display builder (`display::build`) takes `&Buffer` and resolves faces per char from text properties. The core has no async runtime, so parsing on the core thread is the only option until the syntax service exists.

## Goals / Non-Goals

**Goals:** a reusable pipeline (language table, incremental tree, visible-range query, palette) with JS/TS/TSX as its first languages.

**Non-Goals:** injections (JSDoc, regex, template literals), locals-based highlighting, re-picking the language when a buffer's file changes (`write-file` to a new name), off-thread parsing, other languages.

## Decisions

- **Tree in the buffer.** `Buffer` gets an `Option<Syntax>` holding the parser, tree and a stale flag, set when a file buffer is created. `raw_insert`/`raw_delete` call `Tree::edit` with byte offsets and points computed from the rope, so every path (undo, revert) is covered in one place. Alternative: a side table in `Core` keyed by buffer — every edit path would have to notify it.
- **Reparse lazily, before drawing.** Edits only mark the tree stale; `window_display` reparses a stale buffer (`parse_with_options` fed from rope chunks, with the edited old tree) before building. A revert applying hundreds of hunks then costs one reparse, not hundreds. `window_display` takes `&mut Buffers` for this.
- **Language picked once.** On creating a file buffer (visits and previews); `tree-sitter-javascript`'s `highlights-params.scm` is not exported by the crate and is left out.
- **Query only the visible lines.** `QueryCursor::set_byte_range` over the window's line range. Captures are painted into a per-byte colour array for that range: longer captures first, and for equal ranges lower pattern indices first, so the innermost node wins and the later pattern wins for the same node, as Zed and Helix resolve them (the bundled queries put catch-alls like `(identifier) @variable` first). The result is a sorted list of disjoint char ranges with colours, looked up per char by binary search.
- **Queries combined by concatenation.** TS = JS highlights + TS highlights; TSX additionally JSX highlights, JavaScript's own `tree-sitter.json` order (highlights, then JSX) with TypeScript's query last, as it extends JavaScript's; JavaScript itself uses JS + JSX, since its grammar parses JSX in `.js` files. Compiled once per language in a `LazyLock`.
- **Palette.** `Theme.syntax: BTreeMap<String, u32>`; settings tables already merge key by key, so user entries override single colours. Bundled colours follow VS Code Dark+, matching the bundled theme. Colours only; no bold/italic.
- **Precedence.** Syntax gives the base foreground; face properties then merge over it as today, and the region background last.

## Risks / Trade-offs

- [Parsing a large file blocks the core thread on visit and on big edits] → `shortcut:` comment; move to the syntax service when it lags.
- [A very long line (minified file) makes the visible byte range large] → linear cost per frame, accepted.
- [With folds, lines past the window's buffer lines are drawn uncoloured] → folds are rare; same shortcut as the property filter.
