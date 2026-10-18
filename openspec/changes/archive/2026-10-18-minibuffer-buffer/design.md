# Design

## Context

Every command acts on `Core::window`, the selected window, and its buffer; keymaps layer the buffer's minor modes, major mode, then `global`. `Ctx::try_call` already checks `ArgSpec::prompts` against `Args::strings` and fails when one is missing. The frontend draws the echo area as one text line under the windows.

## Goals / Non-Goals

**Goals:** a real buffer and window for the minibuffer, so every editing command works in it unchanged.

**Non-Goals:** completion (#39), recursive minibuffers, leaving the minibuffer for another window while it stays active, multi-line display, IME marked text drawn in the echo line, a default value for `M-n`.

## Decisions

- **The minibuffer window is selected through the existing `select`.** It lives in `others` under a reserved `WindowId(usize::MAX)` outside the split tree; opening inserts it and selects it, closing selects the window it came from and releases it. Every command then edits it with no changes, and wheel scrolling (select, scroll, select back) keeps working. Alternative: a separate field checked by every command — touches every caller.
- **A fresh buffer per prompt**, created on open and killed on close, so undo history and markers never leak between prompts. Its major mode `minibuffer-mode` names a `[minibuffer-mode]` table in `keymap.toml`.
- **The prompt stays outside the buffer text.** Emacs keeps it as read-only text; we have no read-only property, and keeping it out means point can't enter it and the returned string is the whole text.
- **Prompting is a continuation.** `try_call` stores the command name and args (prefix included) in the minibuffer state; `exit-minibuffer` closes it, appends the text to the strings, sets `this_command` to the command and calls it again through `Ctx::call`, so chained prompts, errors and mark deactivation work as for any call.
- **History keyed by prompt text**, newest first, capped at 100 (Emacs's `history-length`), skipping empty input and repeats of the newest as Emacs's `read-from-minibuffer` does. Different prompts get different histories until commands want to share one.
- **Window commands fail while the minibuffer is selected** with Emacs's messages (`Attempt to split minibuffer window`, `Can't expand minibuffer to full frame`); `other-window` fails too, since leaving it active but unselected is out of scope. `delete-window` already fails because the tree has no such leaf. A click aborts the minibuffer first for the same reason.
- **Display.** `FrameDisplay` gains `minibuffer: Option<Minibuffer>` with the prompt and a one-row `WindowDisplay` built like any window's; `Core::minibuffer()` gives the same to the frontend's render, which draws prompt, text before the cursor, a 2 px cursor and the rest in the echo line, with the message as ` [msg]` after it (Emacs's `minibuffer-message` style).

## Risks / Trade-offs

- [A click loses the typed input] → Acceptable until commands need to leave the minibuffer; noted as a shortcut.
- [IME composition at a prompt is not drawn and the IME window has no cursor box to sit at] → Shortcut comment; macOS IME check added to the M0 gate list.
- [No command prompts yet, so the frontend part can't be seen in the app] → Automated core and GPUI tests now; the visual check goes to the M0 gate's macOS list with `M-x`.
