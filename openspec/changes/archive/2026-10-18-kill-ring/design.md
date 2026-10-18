# Design

## Context

The core is a GPUI-free state machine: input arrives as `Msg`, platform work leaves as `Effect`. Commands are `fn(&mut Ctx, Args)` with an `ArgSpec`; `Ctx::call` fills the region from point and mark when the spec asks. `last_command`/`this_command` are tracked per key sequence, and a handler may overwrite `this_command` (as `pop_to_mark` does). The mark is a `Stay` marker, so text inserted at it lands after it. The frontend (`crates/aiditor/src/main.rs`) has the GPUI `cx`, which owns the clipboard (`read_from_clipboard`, `write_to_clipboard`).

## Goals / Non-Goals

**Goals:** the ring, the five commands plus the two word kills, appending, and #30's interprogram rules, all testable headless with a fake clipboard.

**Non-Goals:** `yank-from-kill-ring` (the minibuffer `M-y` of Emacs 28, no minibuffer yet), `kill-whole-line`, `kill-do-not-save-duplicates`, `C-M-w` (append-next-kill), read-only buffers, rich text, PRIMARY, drawing the region (#11).

## Decisions

- **Ring in `Core`.** `kill_ring: VecDeque<String>` (front newest, capped at 60), `yank_index: usize`, and `clipboard_seen: Option<String>`: the clipboard text the editor last wrote or took in. Text on the clipboard that differs from `clipboard_seen` is foreign. This matches Emacs's `gui--last-selected-text` check without comparing against the whole ring.
- **Reading the clipboard lazily.** The frontend can't know which command a key runs, and reading the platform clipboard on every key costs a round trip on X11/Wayland. So `ArgSpec` gets `clipboard: bool`, and `Core::wants_clipboard(key)` looks up `pending + key` through the same layers as the key loop and answers whether it ends in such a command. When it does, the frontend sends `Msg::Clipboard(text)` before `Msg::Key`. The core keeps that text in `clipboard: Option<String>` for the key that follows and clears it once the key is handled, so stale text is never used. Alternative rejected: read before every command key (cost per key), or have the core request a read as an effect and resume (needs continuations).
- **Writing the clipboard.** `Effect::SetClipboard(String)`; the frontend writes `ClipboardItem::new_string`. The core sets `clipboard_seen` to the same text when it emits the effect.
- **Appending.** As in Emacs, every kill command sets `this_command = "kill-region"`, and a kill joins the newest entry when `last_command == Some("kill-region")`. `kill-ring-save` joins the same way but keeps its own name, so a kill after it starts a new entry (Emacs's behavior). Direction: backward for `M-DEL` and `M-d`/`C-k` with a negative or zero count, and for `C-w`/`M-w` with point before the mark, matching Emacs's `(< end beg)`.
- **One kill helper.** `Ctx::kill(range, backward, copy)` reads the text, deletes it unless `copy` (`kill-ring-save`), pushes or joins, and emits the effect; the word kills and `kill-line` reuse `delete_to`'s range logic. Foreign clipboard text is pushed first by `Ctx::take_clipboard()`, shared by kills and `yank`; it returns whether it pushed, and a push disables joining.
- **Yank.** `yank` pushes the mark at point inactive and silently (`window.push_mark(.., false)`, not `Ctx::push_mark`, which messages), inserts the entry, and for `C-u` swaps point and mark. `yank-pop` checks `last_command == Some("yank")`, deletes between point and mark, rotates `yank_index` with `rem_euclid`, inserts, and sets `this_command = "yank"`.
- **Region commands.** `kill-region` and `kill-ring-save` set `spec.region`, so the existing "mark is not set" error comes for free. The builtin table stays a list of `(name, handler)`; the few non-default specs are set right after it.

## Risks / Trade-offs

- [An empty kill (`C-w` with point at the mark, `C-u 0 C-k` at a line start) pushes `""` and empties the clipboard] → that is what Emacs does; guard it if it bites.

- [`wants_clipboard` misses a kill bound to a plain char typed via `Msg::Insert`] → that kill just doesn't see foreign clipboard text; acceptable, nothing binds one.
- [A clipboard that holds only non-text] → the frontend sends nothing, as if empty.
- [The harness must mirror the frontend's clipboard step] → it does so in `keys`, the one entry point tests use for key scripts.
