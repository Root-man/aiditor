# Proposal

## Why

`git commit` and `git rebase -i` in a terminal buffer launch `$EDITOR`, which today is whatever the user's shell sets, so the edit happens in a terminal program instead of in aiditor. #253 decided on an edit server over a Unix socket and a client built into the `aiditor` binary. This change builds the transport for it: the server, the client and the terminal environment. #255 then opens the files and adds the finish and abort keys.

## What Changes

- Each editor listens on its own Unix socket, `aiditor/<pid>.sock` under `$XDG_RUNTIME_DIR` (Linux) or `$TMPDIR` (macOS). The socket is 0600 in a 0700 directory and is removed on quit.
- `aiditor --wait [+LINE[:COL]] FILE...` is the client. It runs before GPUI starts, sends one JSON request line, blocks for the one-line reply and exits with its code. It exits 2 with a reason on stderr when there is no socket or it can't connect, and 1 when the connection drops.
- Terminal buffers get `AIDITOR_SOCKET`, `AIDITOR_TERMINAL`, and `GIT_EDITOR`, `EDITOR` and `VISUAL` set to the shell-quoted `<current_exe> --wait`.
- A request reaches the core as a message with the terminal id and the files: absolute paths, each with an optional line and column. The core's reply goes back over the same connection. Malformed requests are refused by the server, and unknown terminals by the core. Neither crashes the editor.
- Until #255, the core refuses every valid request with "Editing from terminals isn't built yet", so git aborts at once instead of hanging.

## Capabilities

### New Capabilities
- `edit-server`: the editor's socket, the `--wait` client, the request and reply protocol, and how requests reach the core.

### Modified Capabilities
- `terminal-processes`: terminals get the client and socket variables in their environment.

## Impact

- `aiditor-services`: a new `server` module; changes to `Runner` (the reply effect, removing the socket on shutdown) and to `Terminals` (the client variables).
- `aiditor-core`: a new `Msg::EditRequest` and `Effect::EditReply`, handled in dispatch and by the harness.
- `aiditor`: the `--wait` mode in `main.rs`, a socket path in `Args`, and the server started in `Editor::new`.
- No new dependencies. Between this change and #255, `git commit` in a terminal fails fast with that message.
