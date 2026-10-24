# Design

## Context

`Terminal::copy_text` already maps each mark to a (line, char column) in copy mode's text, wrapped rows joined, and folds `A`/`B` into one input start per command for prompt jumps; the buffer keeps those as sorted char indices. The text is frozen while output keeps feeding the grid, so positions must be computed on entry.

## Goals / Non-Goals

**Goals:** select one command's output from its marks, in the frozen text, with `RET` copying it.

**Non-Goals:** the command in char mode; selecting the command line with its output; an output capture API for agents.

## Decisions

- **`copy_text` returns commands, not prompts.** Each is `{ start, input, output, end }` as char indices (`start` is `A` else `B`, `input` is `B` else `A`, `output` and `end` optional). The same arrival-order pass that pairs `A` with `B` gives `C` and `D` to the latest command; a `D` with no `C` before it (an empty command line) is ignored. Char indices come from the line starts the faces already use, clamped to the line, so `vterm_copy_mode` no longer converts. The buffer keeps the commands; prompt jumps take their sorted, deduplicated `input`s.
- **Command at point** is the last command with `start <= point`; its output ends at `D`, else the next command's `start`, else the text's end. A trailing `\n` before that end is dropped, so the copy is the output's lines as `RET` copies a line.
- **Point at the output's start, mark at its end,** pushed and active, as `mark-paragraph` does; `vterm-copy-mode-done` already copies an active region.
- **Key `C-c C-r`,** comint's `comint-show-output` key, in the `vterm-copy-mode` keymap.

## Risks / Trade-offs

- [Marks dropped by a full scrollback or ED 3] → no commands, so it echoes `No command at point`.
- [A `D` on a dropped blank row below the cursor] → the output runs to the next prompt or the text's end, which is the same text.
