# Design

## Context

`Core::minibuffer` formats the count and the frontend's `echo_line` prepends it to the prompt, so the core alone decides its width. The usable columns already subtract `count.len()`. `Completion.rows` is the existing grow-only value for this minibuffer's list. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:** the prompt holds its column through normal count changes, and also past 6 columns, since grep lists up to 10,000 hits.

**Non-Goals:** changing the selection logic, which already holds across appended batches (`set_items` only selects 0 on `replace`).

## Decisions

- **Right-aligned, not left-aligned.** Vertico's default is `%-6s `, but the approved design's frame 3 draws `  3/48 `, and AGENTS.md makes the design binding. Right-aligning also keeps the `/` nearer the prompt.
- **Grow-only width, one `usize` on `Completion` starting at 6.** A fixed `{:>6}` is what the issue suggests, yet `1/10000` is 7 columns, so grep would still jump. Sizing for the largest possible count isn't possible: filtered and matcher lists have no bound. Grow-only follows `rows` and costs one field. It moves the prompt at most once per new width, and never back.
- **Padding lives in the count string.** `display::Minibuffer.count` keeps its type, the frontend stays unchanged, and `echo_line`'s point offset stays right because the padding is ASCII.

## Risks / Trade-offs

- [The prompt still moves once when a count first passes the current width] → Accepted; reserving more columns would waste them on every prompt.
- [Every completing prompt loses up to 3 columns of text] → Negligible next to the frame's width, and it's the design's form.
