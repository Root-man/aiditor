# Design

## Context

While the minibuffer is open it is the selected window, and the window it was opened from (`Minibuffer::from`) sits in `Core::others`. A `Window` keeps point, mark and mark ring as markers in its buffer, so it can't be copied; `Window::show` releases the old buffer's markers, and `Window::release` sets the buffer's remembered point. Every way the minibuffer closes goes through `Core::close_minibuffer`.

## Goals / Non-Goals

**Goals:** one hook in the command spec that the finder (#70) and grep can reuse; exact restoration.

**Non-Goals:** previewing a position inside a buffer (grep needs it; it extends the hook then); creating buffers for preview (the finder decides that in #70); debouncing.

## Decisions

- **Swap the window, don't mutate it.** On a preview the original `Window` is moved out of `others[from]` into the `Minibuffer` (`saved`) and a fresh `Window::new` on the candidate's buffer takes its slot. Restoring moves the original back and releases the preview window. The original's markers stay in its buffer the whole time, so point, mark, ring and scroll come back untouched. Alternative: record point/mark/ring/top and replay them with `show` — more code, and `show` drops the mark ring.
- **Hook shape `fn(&Core, &str) -> Option<BufferId>`** in `ArgSpec::preview`. It only names a buffer, so a hook can't raise the buffer, run commands or change state; the core owns the window swap. `&Core` because nothing needs `&mut` yet.
- **Where it runs:** `Core::preview` after `poll_completion` in `handle`, comparing the selected candidate with the last previewed one (`previewed`), so typing, `C-n`/`C-p` and matches arriving on the repoll timer all update it, and an unchanged selection costs one string compare. Each change restores first, then previews again unless the hook gives no buffer or the original's buffer.
- **Restore in `close_minibuffer`**, before selecting `from` again, so `C-g`, `RET`, a click and a workspace switch all restore. `RET` thus runs the command from the original window, and `switch-to-buffer`'s MRU raise and remembered points behave as without a preview.

## Risks / Trade-offs

- [Release sets the previewed buffer's remembered point to the preview window's point] → the preview window never moves point, so it writes back the same position.
- [A preview window starts at line 0 with one row until the next redisplay] → redisplay scrolls it to point before it's drawn.
