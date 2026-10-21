# Tasks

## 1. Scope buffer lookup and completion to the workspace

- [x] 1.1 Make the buffer-name lookup used by `switch-to-buffer`, `switch-to-buffer-other-window` and `kill-buffer` find only the current workspace's buffers and `*Messages*`, and filter `buffer_names` the same way; verify the existing buffer-switching tests still pass
- [x] 1.2 Add core tests for the new scenarios (other workspace's buffers not listed by `C-x b`; `C-x b x RET` makes `x<2>` here and leaves `x` alone; `C-x k x RET` fails `No such buffer x`; `C-x 4 b` lists no other workspace's buffer); verify `cargo test -p aiditor-core` passes
- [x] 1.3 Update docs/editor.md Workspaces: record the scoping as built in #66 and drop the "Not yet: `C-x b` lists every workspace's buffers" note; verify by reading the section

## Workflow follow-up

- Archive the change after review.
