# Proposal

## Why

#254 built the edit server and the `aiditor --wait` client, but the core turns every request down, so `git commit` in a terminal buffer fails at once. This change does what #253 decided: the requested files open in the terminal's workspace, the user finishes or aborts the edit with keys, and the client's exit code tells git whether to go ahead.

## What Changes

- An edit request for a live terminal opens each file in that terminal's workspace. A buffer already visiting the file is reused. Each file's buffer gets the `server-edit` minor mode. Once every file is read, the first one is shown in the window showing the terminal, or in the selected window if none shows it. A `+LINE[:COL]` puts point there.
- `C-c C-c` and `C-x #` (`server-edit`) finish the current buffer's edit. The buffer is saved first if it is modified, and finishing waits for that save. A save that fails leaves the edit waiting. When every file of a client is finished, the client gets code 0.
- `C-c C-k` (`server-edit-abort`) aborts the client without saving and replies code 1 with `Edit aborted`.
- Killing an edit buffer or its terminal aborts the client. So do killing the workspace and quitting: quitting drops the socket, and the client exits 1. A file that can't be read or decoded aborts it too, with the reason.
- After finishing or aborting, the window shows the client's next file, or else the terminal. A buffer opened for the edit is killed; one that was already open stays.
- If the client goes away first (`^C`), its buffers lose `server-edit`. Those opened for the edit are killed, unless they have unsaved edits. The client's path in `$EDITOR` is quoted only when it needs to be, so git's error doesn't show it quoted twice. To support that, the server tells the core when a client hangs up.
- Defaults picked here:
  - A request for a terminal in another workspace opens its files in that workspace's saved windows, and the echo area says so.
  - With several files, finishing one shows the next.
- **BREAKING** (internal): the "Editing waits for the finish and abort keys" requirement is removed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edit-server`: requests open files, and the finish, abort, kill and hang-up rules decide the reply. The server reports clients that hang up.

## Impact

- `aiditor-core`:
  - A new `edits` module.
  - `Msg::EditGone`.
  - `server-edit` and `server-edit-abort`, with a `[server-edit]` keymap section.
  - Hooks in `kill_buffer`, `on_opened` and `on_saved`.
  - `visit` is split so a file buffer can be made without being shown.
- `aiditor-services`: on hang-up, `server.rs` sends `Msg::EditGone`.
- `docs/editor.md`: the `$EDITOR` paragraph says what was built.
