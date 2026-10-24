# Design

## Context

Keys reach the core as `Msg::Key` and typed text as `Msg::Insert` (one plain key per char), both through `Ctx::key`, which looks the pending sequence up in the layered keymaps. Terminal buffers are read-only buffers whose `Content::Terminal` holds the alacritty `Term`; `Core::terminals` maps each live `TerminalId` to its buffer.

## Decisions

- **Dispatch in `Ctx::key`.** When the current buffer is a live terminal and nothing is pending, a key other than `C-x`, `C-c`, `M-x` or a super key is encoded and pushed as `TerminalOp::Write`, and `key` returns. Everything else falls through unchanged, so prefixes, `C-g` under a prefix and the minibuffer (its own current buffer) work as before. Alternative: a catch-all keymap binding per key — rejected, keymaps can't express "every key".
- **`C-c C-c` is a keymap binding.** Terminal buffers get major mode `vterm-mode`; the bundled `[vterm-mode]` keymap binds `C-c C-c` to a new `vterm-send-C-c` command. The user can rebind it, and `C-c` becomes a prefix only in terminals.
- **Encoding lives on `Terminal`** (`Terminal::encode(key) -> Option<Vec<u8>>`), since it reads DECCKM from the term's mode. It follows Zed's table but is computed: control byte `c & 0x1f`, ESC prefix for meta, modifier code `1 + shift + 2·meta + 4·ctrl`.
- **`Msg::Insert` writes the whole text** to a terminal in one write, ignoring `replace`, instead of one write per char.
- **New key codes** `Home, End, PageUp, PageDown, Delete, F(u8)` use Emacs's names (`<prior>`/`<next>` for pages).

## Risks / Trade-offs

- [A raw `C-x`/`M-x` never reaches programs] → decided in #89; a send-next-key command can come later.
- [Keys without an encoding are silently dropped] → matches a terminal; the encoder covers what xterm sends.
