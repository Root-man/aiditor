# Design

## Context

Tool calls are kept per session in `tools` (by id), each block rewritten in place by `show_tool`. Streamed message blocks are re-rendered per frame from `Core::frame` through `render_streams`. `_meta` on a `tool_call` survives in `ToolCall::meta`; `ToolCall::update` does not merge an update's `_meta`.

## Goals / Non-Goals

**Goals:** show command output and a failed exit in the call that ran it, rendering output-only updates at most once per frame.

**Non-Goals:** a terminal buffer or VT emulation for agent commands; interpreting `\r` as a line overwrite; truncating long output (tool content stays uncut).

## Decisions

- **Terminal state lives on the `Tool`** (`term: Option<Term>` with id, raw output, exit text, and a `dirty` flag), not in a separate map. The call that announced the terminal is the only one that shows it; output for another call's terminal is found by scanning the session's tools, which are few.
- **Store raw, strip at render.** Chunks may split an escape sequence, so stripping runs over the whole output with the `vte` parser alacritty_terminal re-exports (already used by `terminal.rs`), keeping printed chars, `\n` and `\t`. Alternative: a hand-written regex stripper, which misses split and less common sequences.
- **Output-only updates are deferred.** `_meta` handling marks the tool dirty; `render_streams` re-shows dirty tools when the frame is built. Updates with fields still rewrite at once, so status changes, permissions referencing titles and `TAB` behave as before; the block's stored length always matches the rope, so a stale output until the frame is harmless.
- **Exit text is computed once** (`exit N` / `signal S`) and appended inside the header's dim suffix.

## Risks / Trade-offs

- [Codex's output written before its `tool_call` is lost] → adapter bug, documented in #100; nothing to do client-side.
- [`\r` progress lines concatenate] → `shortcut:` comment; upgrade to a line-overwrite pass if real output looks wrong.
- [Re-rendering a long output each frame while streaming costs O(output)] → acceptable for command output sizes seen; flood rules already cap frames.
