# Tasks

## 1. Links in rendering

- [x] 1.1 Add path tokenizing and an `is_file` test to `markdown::render`, faces in `link` + underline; unit tests for existing, missing and punctuated tokens pass
- [x] 1.2 Resolve a token or path against the index roots (relative to any root or absolute under one, optional `:line[:col]`); unit test covers each form
- [x] 1.3 Render agent messages with links from the session project's index, other render calls with none; harness test shows an existing path linked and a missing one plain
- [x] 1.4 Keep a tool call's link on the `Tool` and face its header target; harness test shows a read of a project file linked and `/etc/hosts` plain

## 2. RET

- [x] 2.1 Extract project grep's visit-at-line into a shared helper; existing grep tests pass
- [x] 2.2 `RET` on a link in a message or header visits the file at its line in another window; elsewhere on a header still opens the output; harness tests cover both
- [x] 2.3 Record the behaviour in `docs/editor.md`'s ACP section

## Workflow follow-up

- Archive the change after review.
