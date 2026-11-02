# Tasks

## 1. Messages in the core

- [x] 1.1 Add `Message` and `Session::messages`; replace `output`'s `chunk` argument with `Block`, keep the kind on `Stream`, push or grow messages; verify with core tests for ids on live and replayed chunks and a new id starting a block
- [x] 1.2 Record a sent prompt as an id-less message in `agent_send_input`; verify with a core test of its order after an agent message

## 2. Motion and keys

- [x] 2.1 Add `agent-next-message` and `agent-previous-message` with counts and `No message found`, bound to `C-c C-n` and `C-c C-p`; verify with core tests through both keys past a tool call and a thought
- [x] 2.2 Move `agent-reject-once` to `C-c C-k`; update the existing tests and verify `C-c C-k` rejects a request

## 3. The bar

- [x] 3.1 Add `Decoration::Bar`, `Session::message_lines` and `display::mark_message`, called from the frame for session buffers; verify with a frame test that the bar follows point, stays past a tool call and differs per window
- [x] 3.2 Paint the bar 3 px wide in `element.rs`; verify in the running app against frame 6

## 4. Docs

- [x] 4.1 Update `docs/editor.md`: the protocol table, the permission text and a built-in note for #363; verify no doc names `C-c C-n` for rejecting
