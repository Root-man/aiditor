# Tasks

## 1. Model selection

- [x] 1.1 Generalise the mode option lookup by category and `mode_sent` to `sent`; keep cycle-mode's tests passing.
- [x] 1.2 Add `agent-select-model` with names, annotations and the current value preselected; verify flat and grouped options, sending, the mode line following the answer, a refusal note, and the not-ready, ended and no-model echoes with harness tests.

## 2. Fake agent and docs

- [x] 2.1 Advertise and accept a `model` option in the fake agent, its mode and model kept independently; verify with a services test that selects a model live.
- [x] 2.2 Record the command in `docs/editor.md` and the fake agent's table.
- [x] 2.3 Run fmt, workspace clippy/tests and remaining CI checks.
