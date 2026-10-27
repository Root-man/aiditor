# Tasks

## 1. Core

- [x] 1.1 Keep pending permission requests per session and show each as a block above the prompt line (plan renderer generalised to rows); other requests and requests without a ready session are refused; core tests cover the block's text and properties, a title from the shown call or the id, and refusals
- [x] 1.2 Add `agent-allow-once` (`C-c C-y`) and `agent-reject-once` (`C-c C-n`) and `RET` on an option line in `agent-send-input`, each sending the `selected` outcome and rewriting the block to its answer line; core tests cover allow, reject, RET, oldest first, nothing pending and no option of the kind
- [x] 1.3 `agent-cancel` answers pending requests `cancelled` and an exit drops them, both rewriting the blocks; core tests cover both

## 2. End to end and docs

- [x] 2.1 Scenario test: the fake agent asks three times and `C-c C-y`, `C-c C-n` and `RET` on an option send the matching outcomes, seen in its stderr
- [x] 2.2 Transcript replay answers each permission request with `C-c C-y` and asserts an answer line per recorded request
- [x] 2.3 Record the keys and behaviour in docs/editor.md, and verify with `cargo fmt`, `clippy -D warnings` and `cargo test --workspace`
