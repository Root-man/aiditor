# Design

## Context

`Core` holds one current buffer (`*scratch*`), a point, timers, and a hardcoded self-insert in `Ctx::key`. `Effect` is empty. `Buffers::create_for_file`, `Buffer::{insert, delete, line_col, char_idx, set_saved}` already exist. `main.rs` opens a GPUI window whose `Editor` view routes every message through `Editor::handle` (which matches effects and re-arms the timer) and renders `core.text()` in a plain div. The GPUI spike showed how to take raw keys with `on_key_down` and draw lines.

## Goals / Non-Goals

**Goals:**
- Smallest app that edits and saves a real file, keeping the architecture: the core does no I/O, the app carries out effects.

**Non-Goals:**
- Key trie, command registry, `Msg::Insert`/IME (#12), display builder and custom text element, file service with hash check (#46), goal column, horizontal scrolling of long lines.

## Decisions

**Opening is a direct call, saving is an effect.** `main` reads the file before the window opens (so an unreadable file exits before any UI) and calls `Core::open(path, text)`, which creates the buffer with `create_for_file`, makes it current and puts point at 0. A missing file (`ErrorKind::NotFound`) opens with empty text. A message for opening was rejected: nothing is running yet when `main` reads the file, and #46 replaces this with a service anyway.

`C-x C-s` pushes `Effect::Save { buffer: BufferId, path: PathBuf, text: Rope }` (a rope clone shares nodes, so it is cheap). The app writes it and, on success, handles `Msg::Saved(buffer)`, which calls `set_saved` on that buffer if it still exists. On failure it logs with `tracing::error!` and the buffer stays modified. `Saved` comes back through `Editor::handle` directly, not the channel, because the write is synchronous here. Marking the buffer saved when the effect is emitted was rejected: a failed write would then look saved and could lose edits on quit.

**The prefix is a field.** `Core` gets `prefix_c_x: bool`. `C-x` sets it; the next key clears it and saves only if it is `C-s`. Shortcut, removed by the key trie.

**Editing keys in `Ctx::key`.** One match on the key: char/`RET` insert (existing), `Backspace` deletes `point-1..point`, `Left`/`Right` step point within `0..=len_chars`, `Up`/`Down` use `line_col` and `char_idx(line ± 1, col)` (which already clamps the column). `KeyCode` gains `Left`, `Right`, `Up`, `Down`; `parse_keys` names them `<left>`, `<right>`, `<up>`, `<down>`, as Emacs's `kbd` does.

**Harness runs effects like the app.** `Harness::keys` writes each `Save` with `std::fs::write` and handles `Msg::Saved` on success, so key-script tests check the file on disk.

**Frontend keys.** `on_key_down` on a focused div maps a `Keystroke`: named keys `enter`, `backspace`, `tab`, `escape`, `left`, `right`, `up`, `down`; with Ctrl or Alt held the key's own `key` char with `ctrl`/`meta` set; otherwise each char of `key_char` as a plain key. Every mapped key is consumed. This breaks dead keys and IME, which #12 fixes by letting typing reach the input handler; recorded as a `shortcut:` comment. Cmd keys are left alone.

**Drawing with divs.** Render takes rows from `window.viewport_size()` and the line height, moves a `top` line on `Editor` just enough to keep point's line visible, and draws lines `top..top+rows` as fixed-height divs in a monospace font. The cursor line is a flex row: text before point, a 2px bar, text after. The spike's custom element is more code than the skeleton needs; the display-model renderer replaces this. `window.set_window_title` shows the buffer name and ` *` when modified.

## Risks / Trade-offs

- [`std::fs::write` truncates then writes, so a crash mid-write can lose the file] → Acceptable for the skeleton; #46 moves saving into a service. Shortcut comment at the write.
- [Saving on the UI thread blocks it for large files] → Same fix, #46.
- [Tabs and very long lines draw however GPUI shapes them, with no horizontal scroll] → Display builder's job.
- [A panic in key handling aborts on macOS] → The new core code returns early on errors instead of unwrapping; panic containment is #52.
