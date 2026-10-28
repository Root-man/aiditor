# Tasks

## 1. Core

- [x] 1.1 Replace `plain` with a pass that returns the plain text and SGR faces (16 theme colours fg/bg, bold, underline, resets)
- [x] 1.2 `tool_content` returns the faces of terminal output and text items, offset into the content; the `Tool` keeps them and `render_tool` adds them when open
- [x] 1.3 Core unit tests: colours, bright colours, bold/underline and reset, 256/truecolour drop, faces land on the right chars in an open call, copied text has no escapes (cargo test -p aiditor-core agents)

## 2. Docs

- [x] 2.1 Record the built behavior under Agent command output in `docs/editor.md`
