# Design

## Context

`Core::create_terminal` makes a terminal buffer with a grid and maps a `TerminalId` to it; `Msg::TerminalOutput` feeds it. `TerminalSpawned` and `TerminalExited` are ignored. Buffers are killed through `Core::kill_buffer`, which has no `Ctx` and so cannot push effects; it runs from `kill-buffer`, workspace kills and the workspace list. The core knows the frame size (`Msg::Resize`) and lays out windows itself (`Tree::layout`). The frontend already runs every `TerminalOp` and kills every terminal on `Effect::Quit`.

## Goals / Non-Goals

**Goals:** one place that keeps terminals consistent with buffers and windows, whatever path changed them.

**Non-Goals:** sizing from windows of inactive workspaces (a terminal belongs to one workspace, whose windows are only laid out while it is current); asking before a workspace kill kills terminals (the issue only asks for `kill-buffer` and quitting).

## Decisions

- **A post-message pass, `Core::sync_terminals`, at the end of `Core::handle`**, like `watch`. It drops each terminal whose buffer is gone and sends `TerminalOp::Kill` for it, then resizes each live grid whose smallest window's size differs from the grid's and sends `TerminalOp::Resize`. The grid is the only record of the size, so no extra state. Alternative: push kills from `Core::kill_buffer` via a pending list, and resize from each window command; that touches every caller and misses paths.
- **Window sizes from `Tree::layout`** with the reported frame size, rows `height / line_height - 1` (the mode line) and columns `width / cell_width`, the same arithmetic as `Core::frame`. One helper lists (buffer, rows, cols) per laid-out window; `vterm` uses the selected window's entry for the spawn size, the pass takes the minimum per buffer.
- **Exit and spawn failure remove the map entry first**, then kill the buffer, so the pass sends no `Kill` to a terminal that is already gone.
- **"Live" is having a map entry**: from `vterm` until exit, spawn failure or the buffer's kill.
- **Quit question.** `save_some` stops starting the saves when it goes ahead and returns them; `save-buffers-kill-terminal` and `workspace-kill` start them. Quitting with live terminals then asks its question before the saves. Its answer is stored after a marker string in the command's strings, so `save_some`'s name/answer pairing is not shifted; a declined answer still saves the `Yes` buffers. Alternative: a hidden command for the answer; every command is listed in `M-x`.

## Risks / Trade-offs

- [The pass walks every terminal and lays out windows after every message] → a handful of terminals and windows; the layout is a small tree walk.
- [A terminal not shown keeps an old size until shown again] → Emacs does the same.
