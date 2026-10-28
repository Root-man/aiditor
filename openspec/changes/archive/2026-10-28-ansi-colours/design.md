# Design

## Context

`plain` strips escapes with vte's low-level parser; `tool_content` builds a call's content text and `render_tool` re-derives its markdown faces from the buffer each time it renders (also when `TAB` toggles the fold).

## Goals / Non-Goals

**Goals:** colours, bold and underline from SGR in command output, as faces over plain text.

**Non-Goals:** 256-colour and truecolour (drawn in the default colour), italic, dim, reverse, `\r` as a line overwrite.

## Decisions

- **Parse with alacritty's `vte::ansi::Processor` and a `Handler`**, already used by `terminal.rs`: its `terminal_attribute` hands over parsed `Attr`s, so `38;5;n`/`38;2;r;g;b` are consumed correctly. Alternative: decode SGR params from the low-level parser by hand, more code for the same result.
- **Faces are computed with the text and kept on the `Tool`** (`faces`, content-relative), since the escapes are gone from the buffer by the time `render_tool` runs. They are sorted with the markdown faces by range, which decides the winner where both apply: only text items have markdown faces over coloured chars.
- **A face run never spans a newline**, so no face ends inside a later code line's decoration.
- `\v` and `\f` become newlines, as the processor reports them as line feeds.
- Named colours 0–15 and indexed colours below 16 map to `theme.ansi`; default (`39`/`49`) and anything else to `None`.

## Risks / Trade-offs

- [A synchronized-update escape (`?2026h`) without its end holds the rest of the output in the processor's buffer] → not seen in command output; drop the sync buffer if it shows up.
