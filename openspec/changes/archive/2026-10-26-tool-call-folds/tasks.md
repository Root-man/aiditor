## 1. Buffer

- [x] 1.1 `Buffer::replace_output` replacing a range past read-only without undo, `insert_output` built on it; test that a replacement before the prompt keeps the prompt area's undo working
- [x] 1.2 `add_property` inserts at the sorted place when the order holds; test that a property added mid-list keeps `properties_near` bisecting
- [x] 1.3 `Buffer::unfolded_line`; covered by the motion tests

## 2. Tool calls

- [x] 2.1 Build a call's block text (header, Input, content items, Output, fences) and its properties folded and open; tests in `agents.rs` for a folded call, a call without content, raw output forms and fences
- [x] 2.2 Handle `tool_call` and `tool_call_update` (merge, rewrite in place, unknown ids); tests for status updates under a later message and an unknown id
- [x] 2.3 `agent-toggle-tool-call` on `TAB` in `agent-session-mode`; tests for toggling, staying open across updates and `No tool call here`

## 3. Motion

- [x] 3.1 `next_line` and `Window::scroll` skip folded lines; tests for `C-n`/`C-p` past a folded call and a window keeping the header in view

## 4. Docs

- [x] 4.1 Record the tool call display in `docs/editor.md` under the session buffer
