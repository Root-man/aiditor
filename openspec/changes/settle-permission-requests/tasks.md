## 1. Core

- [x] 1.1 In `agent_request`, answer a permission request from an ended or unknown session with the `cancelled` outcome, without a block
- [x] 1.2 Test: a request after exit and after killing the session buffer is answered `cancelled`; `C-c C-c` with a pending request answers it and ends the turn
- [x] 1.3 Update the core and services refusal tests for an unknown agent

## 2. Docs

- [x] 2.1 Record the behavior in `docs/editor.md`'s session buffer and process notes
