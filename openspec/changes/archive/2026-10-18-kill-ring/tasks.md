# Tasks

## 1. Core kill ring and clipboard plumbing

- [x] 1.1 Add `ArgSpec::clipboard`, `Msg::Clipboard`, `Effect::SetClipboard`, the ring fields and `Core::wants_clipboard`; give the harness a fake clipboard that mirrors the frontend; verify a harness test that `wants_clipboard` is true for `C-y`, after `C-x`-style prefixes only when they end in a clipboard command, and false for `C-f`

## 2. Commands and keys

- [x] 2.1 Implement the kill helper (push, join, prepend, foreign clipboard first, clipboard effect) and `kill-line`, `kill-region`, `kill-ring-save`, and make `kill-word`/`backward-kill-word` use it, dropping the `shortcut:`; verify harness tests for every scenario of the `kill-ring` spec's kill, join and clipboard requirements and the `editing-keys` delta
- [x] 2.2 Implement `yank` and `yank-pop` with Emacs's prefix rules; verify harness tests for the yank and `M-y` scenarios, including foreign clipboard text
- [x] 2.3 Bind `C-k`, `C-w`, `M-w`, `C-y`, `M-y`, `s-x`, `s-c`, `s-v` in `keymap.toml` and update its Cmd comment; verify the key scripts in 2.1 and 2.2 run through the bindings

## 3. Frontend

- [x] 3.1 In `main.rs`, send `Msg::Clipboard` with the clipboard's text before a key `wants_clipboard` accepts, and write the clipboard on `Effect::SetClipboard`; verify with a GPUI `TestAppContext` test that `s-c` on a region fills the clipboard and `s-v` yanks text written to it from outside

## 4. Docs

- [x] 4.1 Update `docs/editor.md` (Kill ring bullet: what was built, the lazy clipboard read; insertion bullet and Cmd keys note no longer say "until the kill ring lands"); verify by reading the edited bullets

## Workflow follow-up

- Verify, commit, archive and ship per the implement skill.
