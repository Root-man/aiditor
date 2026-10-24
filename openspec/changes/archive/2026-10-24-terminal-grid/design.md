# Design

## Context

See proposal.md. The core is a plain state machine with no GPUI or Tokio (CI checks `cargo tree`). Buffers hold `Content::Text(Rope)`, and every text command reads `Buffer::text()`, which returns `&Rope`. `Msg::TerminalOutput` is dispatched to nothing today.

## Goals / Non-Goals

**Goals:** the emulator in the core behind an interface of three calls; terminal buffers as a content variant; output and replies routed by `TerminalId`.

**Non-Goals:** rendering the grid, key encoding, copy mode text, resize, spawning or killing from the core.

## Decisions

- **`alacritty_terminal` 0.26 from crates.io, default features off.** Already decided in docs/editor.md; Zed's fork adds nothing this needs. Its dependencies include no GPUI or Tokio.
- **A `terminal::Terminal` struct as the interface**: `new(rows, cols)`, `feed(&mut self, bytes) -> Vec<u8>` returning the replies, and `screen() -> String`. No trait: one implementation, and moving the emulator later changes the struct's insides, not its callers. Replies come from alacritty's `EventListener`, which takes `&self`, so the listener appends `PtyWrite` text to an `Rc<RefCell<Vec<u8>>>` shared with the struct; `feed` takes it. Every other event (title, bell, clipboard, color and size queries) is dropped.
- **`Content::Terminal(Box<Terminal>)`.** `text()` returns a shared empty rope (a `LazyLock<Rope>`), so no command needs to know. `raw_insert` and `raw_delete` refuse non-empty edits to terminal content with `ReadOnly`, so even the read-only-bypassing paths (`append`, `set_text`, `replace`) cannot touch it; the buffer is also created `read_only`, so commands fail with the usual message.
- **`Core::create_terminal(name, rows, cols) -> (TerminalId, BufferId)`** makes the buffer in the current workspace and records the id in a `BTreeMap<TerminalId, BufferId>`. It emits no spawn: the command that opens a terminal comes later and will pair this with `TerminalOp::Spawn`.
- **Unknown output**: an id not in the map, or one whose buffer was killed, is ignored. Stale map entries stay until terminal lifecycle work removes them with the buffer.
- **Screen snapshot**: each visible row's cells, wide-char spacers skipped and trailing blanks trimmed, joined by `\n`. Good enough for snapshot tests; rendering will read the grid directly.
- **Scrollback**: alacritty's default `Config` (10,000 lines).

## Risks / Trade-offs

- [VT parsing on the UI thread can lag typing during floods] → accepted by docs/editor.md; the interface lets it move later.
- [Ignoring OSC 10/11 color and `CSI 14 t` size queries] → programs that ask get no answer; the ones that matter (vim, fzf) use cursor position and device attributes, which are answered.
