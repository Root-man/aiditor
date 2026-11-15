## 1. Core

- [x] 1.1 Split `visit` into `file_buffer` and the switch; add `Buffers::visiting_in(path, workspace)`
- [x] 1.2 Add the `edits` module: `Client`, `Edit`, `Core::clients`, `edit_request` opening the files, and replacing the refusal in `terminal.rs`
- [x] 1.3 Route client reads in `on_opened`; show the first file once all are open, with its position; echo when the workspace isn't current
- [x] 1.4 `server-edit` and `server-edit-abort` commands, `[server-edit]` keymap section (`C-c C-c`, `C-x #`, `C-c C-k`)
- [x] 1.5 Finish waits on the save through `on_saved`; after finish or abort show the next file or the terminal and kill opened buffers
- [x] 1.6 Abort clients from `Core::kill_buffer` (edit buffers and terminal buffers)
- [x] 1.7 `Msg::EditGone` drops the client and its buffers' `server-edit`
- [x] 1.8 Harness tests: open and show, reuse, position, unreadable file, finish with save, failed save, two files, abort, kill buffer, kill terminal, quit asks, gone, unknown terminal

## 2. Services

- [x] 2.1 `serve` sends `Msg::EditGone` when the client hangs up; extend the server test

## 3. Docs

- [x] 3.1 `docs/editor.md`: mark #255 built and record the defaults (other workspace, several files)
