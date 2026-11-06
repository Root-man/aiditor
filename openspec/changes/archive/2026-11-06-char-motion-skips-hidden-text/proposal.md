# Proposal

## Why

Left and right in an agent session's output moved over a link's full markdown, not what the buffer shows: `forward-char` and `backward-char` stepped through each hidden `[` and `](url)` char, so point seemed stuck for several presses at every link. Folds with a placeholder took as many steps as the chars they hide.

## What Changes

- `forward-char` and `backward-char` move by shown chars: invisible text with no placeholder takes no step, and a fold with a placeholder or a display-property range takes one. Buffers without hidden text keep the old fast path.

## Capabilities

### New Capabilities

### Modified Capabilities
- `motion`: char motion steps over hidden text.

## Impact

- `crates/aiditor-core/src/motion.rs` (`forward_char`).
- `docs/editor.md` Session buffers (motion over folds).
- No API or dependency changes.
