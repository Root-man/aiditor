# Design

## Context

Commands read strings through the minibuffer (`read_string`), and a command can open further prompts itself, as `kill-buffer` does for its confirmation. Saves are async: `Effect::Save` goes to the file service and `Msg::Saved` comes back. The app quits only when its last window closes.

## Decisions

- **Answers carry the buffer they are for.** The handler pushes the buffer's name onto `args.strings` before opening each `Save file` prompt, so the strings are name/answer pairs and the next buffer asked about is the first modified file buffer not yet named. A save finishing mid-prompt cannot shift answers onto the wrong buffer. An odd count after the pairs is the `exit anyway` answer.
- **Owned prompts.** `Minibuffer::prompt` and history keys become `String`, and `read_string` takes a `String`. Each file's prompt then has its own history, which is harmless.
- **Quit state in the core.** After the answers (the `exit anyway` one included), the handler saves each `Yes` buffer that is still modified; a declined exit stops there. Otherwise it keeps their ids in `Core::quitting`. Empty means quit now: `Effect::Quit`. `Msg::Saved` removes the id on success when the buffer is now unmodified, and emits `Effect::Quit` when the set empties; an error or a buffer still modified clears `quitting`, cancelling the quit.
- **Window close.** `on_window_should_close` sends `Msg::Quit` and returns `false`; the core runs the command as if `C-x C-c` were typed. `Effect::Quit` calls `cx.quit()`.
- **`s-q`** joins `C-x C-c`; `s-w` and `s-h` stay unbound until frames exist.

## Risks

- Quitting from inside the minibuffer fails with the existing `Command attempted to use minibuffer while in minibuffer` when something is modified; with nothing modified it quits.
- Edits typed while a chosen save is running leave the buffer modified, which cancels the quit rather than dropping them.
- A second `C-x C-c` while a quit's save still runs saves again, which is refused as a change on disk and cancels the quit; nothing is lost.
