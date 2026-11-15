## 1. Grep service

- [x] 1.1 Add `names: Vec<String>` to `GrepSearch`; project grep passes none
- [x] 1.2 Search only files whose names contain a given name when some are given; service test

## 2. Core

- [x] 2.1 `ProjectGrep` keeps its command and the searched sessions; `poll_grep` and `on_grepped` serve any `grep_hits` command
- [x] 2.2 The text-only filter and snippet for session hits, with unit tests
- [x] 2.3 `agent-sessions-search`: roots, names, prompt, errors; header labels; the preview at the session row; `RET` through the factored visit
- [x] 2.4 Session hits listed without line numbers
- [x] 2.5 `s` in `agent-sessions-mode`, the `search` hint
- [x] 2.6 Harness tests: roots and names, a word only in one session finds it, metadata-only lines dropped, header, preview, `RET` loads

## 3. Docs

- [x] 3.1 Record in `docs/editor.md` how the hits are shown (#487)
