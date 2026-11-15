# Design

## Context

Services run on Tokio and message the core over the shared `mpsc` channel. The core answers with `Effect`s that `Runner` carries out. `Terminals::spawn` builds each PTY's environment with `terminal_env`. The core maps `TerminalId` to its buffer in `core.terminals`. `Args` holds per-run paths that `parse_args` derives, and tests use `Args::default()`, which has none.

## Goals / Non-Goals

**Goals:** the transport, end to end, with untrusted input handled at the server.

**Non-Goals:** opening files, the `server-edit` minor mode, finishing, aborting, and telling the core that a client went away. All of these are #255.

## Decisions

- **Wire format.** The request is `{"cwd": "...", "terminal": N, "args": [...]}`. The server, not the client, parses the `+LINE[:COL]` arguments, so validation lives in one place at the trust boundary. The reply is `{"code": 0|1, "error": "..."?}`. `serde_json` is already a services dependency.
- **The server module** `aiditor_services::server`:
  - `Server::listen(path, tx)` binds a tokio `UnixListener` and spawns its accept loop.
  - Each connection reads one line, capped at 1 MiB. It parses and validates it, takes a request number from a counter, stores a `oneshot` sender in a shared map, and sends `Msg::EditRequest`. It then waits for the reply or for the client's EOF, whichever comes first, and drops the map entry on EOF.
  - `Server::reply` sends on the stored sender.
  - Dropping the `Server` aborts the accept loop and removes the socket file. `Runner::shutdown` drops it.
- **The socket path** comes from `parse_args` as `Args::socket`, like `recovery_dir`. `Editor::new` listens only when it is `Some`, so the GPUI tests, which make many editors in one process, never bind a socket. On Linux without `XDG_RUNTIME_DIR` there is no path. `/tmp/aiditor` is shared between users and another user could plant it, so it is not a fallback. On macOS `std::env::temp_dir()` reads `$TMPDIR`.
- **Permissions.** Create the directory with `DirBuilder::mode(0o700)` and also `set_permissions(0o700)` if it already exists. Remove a stale socket file, bind, then `set_permissions(0o600)`. The 0700 directory already keeps other users out during the moment before the chmod.
- **Environment.** `Terminals` gets a `pub client: Vec<(String, String)>`, which holds the socket and editor variables and is empty without a socket. In `spawn` those variables plus `AIDITOR_TERMINAL=<id>` go after the spawn's extra variables, so they win over both inherited and project variables. The command is `current_exe()`, single-quoted with `'\''` escapes, followed by ` --wait`.
- **Client.** `aiditor_services::server::client(socket, terminal, args) -> i32`, taking `AIDITOR_SOCKET` and `AIDITOR_TERMINAL` from `main` so tests need no process env, uses blocking `std::os::unix::net::UnixStream`. `main` checks `--wait` as the first argument, before tracing and before `parse_args`, and exits with the returned code.
- **Core.** The new message is `Msg::EditRequest { request, terminal, files: Vec<ClientFile> }`, where `ClientFile { path, line: Option<usize>, col: Option<usize> }`. The core answers with `Effect::EditReply { request, result: Result<(), String> }`. The core checks `core.terminals`: an unknown id gets `Err("No terminal N")`, and a known one gets the not-built-yet error. The harness records replies in `h.edit_replies`.

## Risks / Trade-offs

- [`git commit` in a terminal fails until #255 lands] → The message says why, and the user chose this over hanging.
- [A crash leaves `<pid>.sock` behind] → The OS clears the runtime directory, and a later editor with the same pid replaces the file.
- [A client that connects and never writes holds a task] → It is per-user and the cost is tiny. `shortcut:` comment: no read timeout; add one if idle connections ever pile up.
