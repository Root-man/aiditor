# Tasks

## 1. Core messages

- [x] 1.1 Add `ClientFile`, `Msg::EditRequest` and `Effect::EditReply`. The core replies "No terminal N" for unknown terminals and "Editing from terminals isn't built yet" for known ones. The harness records the replies. Verify with core tests for both replies.

## 2. Server and client

- [x] 2.1 Add `aiditor_services::server`: the socket dir and path, `Server::listen` with permissions and stale-file removal, per-connection parsing and validation, `reply`, and removal on drop. Verify with services tests: a request reaches the core channel with absolute paths and positions, the client exits 0 or 1 with the reply, a malformed request is refused and later ones are still served, permissions are 0600 and 0700, and the socket is gone after drop.
- [x] 2.2 Add `server::client`. Verify with services tests: no `AIDITOR_SOCKET` exits 2, a dead socket exits 2, and a dropped connection exits 1.
- [x] 2.3 Add `Effect::EditReply` to `Runner`, and drop the server in `shutdown`. Verify with a runner test that the socket is gone after shutdown.

## 3. Terminal environment

- [x] 3.1 Add the `Terminals::client` variables and `AIDITOR_TERMINAL`, and the shell-quoted `<exe> --wait` command. Verify with a terminal test where `echo $EDITOR $GIT_EDITOR $AIDITOR_TERMINAL` shows the client and the id over an inherited `GIT_EDITOR`.

## 4. App wiring

- [x] 4.1 Run `--wait` as the client in `main.rs` before anything else, and update USAGE. Add `Args::socket` from `parse_args`, and start the server in `Editor::new` and set `terminals.client` from it. Verify with `cargo build`, and by hand that `echo $EDITOR` in a terminal buffer shows the client.
- [x] 4.2 Note in the `$EDITOR` paragraph of `docs/editor.md` that #254 built the transport and that requests are refused until #255. Check the paragraph reads correctly.
