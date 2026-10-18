# Proposal

## Why

Commands can declare prompted strings, but a call without them just fails, so no command can ask the user for input. `M-x`, `find-file` and buffer switching (#39 onward) all need a minibuffer to read from (issue #38).

## What Changes

- A command called with fewer strings than its prompts opens the minibuffer instead of failing: a fresh one-line buffer in its own window, selected while it is active, with the prompt shown before its text.
- `RET` (`exit-minibuffer`) returns the text as the next string and calls the command again, so a command with two prompts asks twice; the prefix argument given before it is kept. `C-g` (`abort-minibuffers`) closes it with `Quit` and runs nothing.
- The minibuffer's buffer has major mode `minibuffer-mode`, whose keymap binds `RET`, `C-g`, `M-p` and `M-n`; every other key falls through to the global keymap, so normal editing works inside it.
- One history per prompt: `M-p` (`previous-history-element`) and `M-n` (`next-history-element`) move through earlier answers, newest first, with Emacs's messages at the ends; going back past the newest restores what was typed.
- No recursive minibuffers: prompting while one is active fails. Window commands that would leave or split it fail while it is selected; a click in a window aborts it.
- The frontend draws the minibuffer in the echo area line: prompt, text and cursor, with any message after it in brackets.

## Capabilities

### New Capabilities
- `minibuffer`: reading a string for a command: prompt, editing, return, abort and history.

### Modified Capabilities
- `commands`: a call missing prompted strings reads them in the minibuffer instead of failing.

## Impact

`aiditor-core` (`lib.rs` command calls, window selection, frame display; `display.rs`; `keymap.toml`), the frontend's echo area line in `aiditor/src/main.rs`, and `docs/editor.md`'s minibuffer bullet. No new dependencies.
