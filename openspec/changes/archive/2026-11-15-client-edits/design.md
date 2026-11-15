# Design

## Context

`Msg::EditRequest { request, terminal, files }` reaches the core, and `Effect::EditReply { request, result }` answers it (#254). Files are read through `FileOp::Open` / `Msg::Opened`, which `on_opened` checks against the single `pending_visit`. Minor modes are names in `Buffer::minor_modes`, and their keymaps are sections of `keymap.toml`. `Core::kill_buffer` is the one place every buffer kill goes through: the kill commands, killing a workspace and a terminal's shell exiting. It can push effects through `released`.

## Goals / Non-Goals

**Goals:**
- Finish, abort, kill and hang-up each answer the client exactly once.
- The core stays free of I/O.

**Non-Goals:**
- `aiditor FILE` without `--wait` opening in the running editor.
- A mode-line marker for `server-edit`.
- Moving point in a saved window's buffer that was already shown elsewhere.
- Switching a saved window of a non-current workspace when its edit ends; it shows the buffer until the user goes there.

## Decisions

- **A new `edits` module in the core** holds `Core::clients: Vec<Client>`. A `Client` has its request, terminal and workspace, and `Edit`s. An `Edit` has a path, a position, `buffer: Option<BufferId>` (`None` while it is read), the visit number of its read, `opened` (whether the edit made the buffer) and `saving`. It goes in a new module because `terminal.rs` is already 2000 lines.
- **Reads use `FileOp::Open`** with a fresh visit number from `Core::visits`. `on_opened` first asks `edits` whether the answer is a client's. If it is, the answer bypasses `pending_visit`, so moving around the editor doesn't drop it.
  - The alternative was the server reading the files itself. That would put file I/O in two places and skip decoding through the core's one path.
- **`visit` is split into `file_buffer` (decode and create, echoing a refusal) and the switch.** Edits make their buffers without showing them, then set the buffer's `workspace` to the client's.
- **Showing:**
  - In the current workspace, the window showing the terminal is selected and `switch_to` shows the buffer.
  - In another workspace, `Window::show` is called on that workspace's saved windows.
  - Point comes from the buffer's own point marker, which `Window::show` copies. So the position is set on the buffer before it is shown.
- **Finishing a modified buffer** calls `save` and sets `saving`. `on_saved` then finishes the edit, or clears `saving` when the save failed or the buffer changed since. This mirrors how `quitting` waits on saves.
- **Kills:**
  - At the start of `Core::kill_buffer`, the killed buffer is taken out of every client before that client is aborted. This keeps the abort's own kills of opened buffers from killing it twice.
  - A terminal buffer's kill aborts the clients of that terminal.
  - Replies go through `released`.
- **Quit replies nothing itself.** The process prompt already asks, since a waiting client means a live terminal. `Runner::shutdown` then drops the server, so the client sees the connection close and exits 1.
- **Hang-up:** `serve`'s branch that sees the connection close already removes the pending sender. It now also sends `Msg::EditGone { request }`.

## Risks / Trade-offs

- **[A buffer in two clients]** (two `git commit`s on one file). Finishing it finishes both edits. Killing it aborts both clients. → Acceptable, and it matches Emacs.
- **[Buffers opened for a client that hangs up mid-read]** Reads still in flight are dropped when they arrive. → Nothing is made for them.
