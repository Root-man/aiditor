# Design

## Context

Commands are `fn(&mut Ctx, Args)` handlers registered by name in `Core::default` and bound in `keymap.toml`. `Ctx::key` still moves point for unbound arrow keys. The window knows its rows only when `Window::scroll(rows)` runs at redisplay, and it clamps `top` so point's line is visible.

## Goals / Non-Goals

**Goals:** the commands in the `motion` spec, as builtins beside `undo` and the prefix commands.

**Non-Goals:** visual-line motion, `scroll-margin`, `scroll-preserve-screen-position`, a mark ring or "Mark set" on `M-<`/`M->`, Emacs's syntax tables for words.

## Decisions

- **Handlers live in `lib.rs`'s builtin table**, with small helpers on `Ctx` like `undo`. A separate module would be the first one for commands; not worth it at this size.
- **Count** is `args.prefix.map_or(1, Prefix::value)`, so `C-u` counts 4 and `C--` counts -1, as `prefix-numeric-value`.
- **Goal column** is a `Core` field set from point's column when `last_command` is not `next-line` or `previous-line`. Columns are chars, matching the display's monospace, tab-less rendering. Alternative, a window field: there is one window, so a core field is as correct and simpler.
- **Ends of the buffer** are handler errors (`Err("End of buffer")`), which `Ctx::call` already puts on the message line, after point has moved as far as it can.
- **Rows**: `Window::scroll` stores `rows` on the window; scrolling commands read it and set `top` directly, and the next redisplay's clamp leaves it alone because the commands keep point visible. Before the first redisplay rows is 1.
- **Recenter cycle**: a `Core` index (middle, top, bottom) reset when `last_command` is not `recenter-top-bottom`. Plain `C-u` recenters to the middle; a number or lone minus is a row.
- **Arrows** are bound in `keymap.toml` and the fallback motion in `Ctx::key` is deleted, so `Up`/`Down`/`Left`/`Right` fall into the `undefined` arm when unbound.
- **No mark on `M-<`/`M->`**: the display draws the region whenever a mark exists, and there is no transient mark or mark ring yet, so setting it would highlight a region the user did not ask for.

## Risks / Trade-offs

- [Char goal column drifts on wide chars and tabs] → acceptable until the display renders them; marked with a `shortcut:` comment.
- [Rows are from the last redisplay, so a resize between redisplay and `C-v` uses the old height] → the app redisplays on resize before the next key.
