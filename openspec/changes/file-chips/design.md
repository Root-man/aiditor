# Design

## Context

The session buffer is a rope with properties; the core never reads the disk. Props are marker ranges, so deleting their text collapses them. The display draws `Invisible` ranges as a placeholder in the default face. Claude's adapter turns a `resource_link` into the text `[@name](file:///path)` and stores it, so a replayed message has that text as its own chunk; it replays an image as an image chunk with base64 data and no path. Codex's adapter was not checked.

## Goals / Non-Goals

**Goals:** the chip as one char everywhere editing is concerned, its drawing, sending, replay, `RET`, echo and missing state.

**Non-Goals:** commands that make chips (`@`, `agent-attach-file`, pastes); the `≡` kind; copying a chip to the system clipboard as its path; percent-encoding URIs.

## Decisions

- **A chip is a private-use char.** The core keeps one chip table, interned by path: entry `i` is the char `U+F0000 + i` (Supplementary Private Use Area-A) with its path, kind and missing state. Every editing command, kill, yank and undo then treat a chip as one char with no code of their own, and a chip yanked into another session still resolves. Alternative: the markdown link text hidden behind a placeholder, as the display model does for markup. Rejected: every motion and deletion command would need intangibility rules, and kill-word would split links.
- **Chips are drawn by a new `Prop::Display(pieces)`.** It replaces its range with text pieces, each in its own faces, so the icon can be in `status-failed` while the rest is struck through. The display handles it as it handles an `Invisible` placeholder (column counting, `AlignRight`, cursor), and the pieces stay separate spans so touching boxes share one outline. Pieces: ` `, the icon, ` <name> `, all over `attachment`; missing adds `strike` to all and `status-failed` to the icon.
- **Painting.** `paint(buffer, range)` puts a `Display` prop on each chip char in range, removing what is on that one char first. The echo and replayed messages are painted once, after markdown. The prompt area, where yank and undo bring chips back, is repainted when a frame is built and the buffer's tick moved since the last paint. A chip's missing state changing repaints every session buffer's chips, a scan of their text, which happens only on file checks.
- **Sending.** The prompt area splits into text blocks around its chips (whitespace-only runs dropped); a file or directory chip is a `resource_link` named after its last path component, URI `file://` + path, a directory's with a trailing `/`; an image chip (extension `png`, `jpg`, `jpeg`, `gif`, `webp`) is an `ImageContent` with its `file://` URI and no data. The agent service fills the data (base64) from the file before sending; a file it can't read answers the prompt with an error, which ends the turn as `Error: …`.
- **Replay.** User chunks join the last block when it is a user block with the same `Some(messageId)`. Each `[@name](file://path)` in a user chunk's text becomes a chip for `path` (a trailing `/` makes a directory); an image chunk becomes a chip with no path, named `image`. The Claude Code CLI saves a mention as the text `@path` (the file goes in a separate attachment entry), so an `@word` that the project's file index resolves, as file links do, becomes a chip too; directories and unindexed files stay text.
- **Missing state.** Interning a new path, or the watcher reporting a path that is or contains a chip's, sends a `FileOp::Check` for those chips; its answer marks a chip missing when the file is gone. A readable directory isn't answered, so it stays present; a gone one answers gone.
- **RET and the echo.** `RET` with point on a chip, before the prompt-send fallback, visits a file in another window as links do, hands an image to the system's default app (`Effect::OpenWithSystem`, GPUI's `open_with_system`) since `find-file` refuses binary files, or opens `find-file` with its input set to the directory; a chip without a path echoes `No file` and a gone one `<path>: no such file`, neither splitting.
- **Words and the cursor.** Word motion takes a chip as a word of its own, so `M-f` stops after it and `M-DEL` kills it alone, without joining text it touches. The display reports how many chars the cursor covers, a whole chip's pieces when point is on one, and the frontend draws the block that wide. After each command, point on a chip with no other echo this command echoes its path.

## Risks / Trade-offs

- [Copying a chip yields a private-use char] → pasting it outside the editor shows a placeholder glyph; acceptable until there is a need.
- [Chips outside watched roots aren't rechecked] → a deleted attachment there stays unstruck until something reports it; shortcut noted.
- [Codex's replay format is unknown] → if it differs, its replayed chips show as text; links in any user text become chips either way.
- [A `Display` prop nested inside a markdown run] → `properties_near` loses its bisect for that buffer; chips sit in plain prompt text in practice.
