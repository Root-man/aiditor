# Tasks

## 1. Core

- [x] 1.1 Store `available_commands_update` on the session, minus `plan`; harness test that updates replace the list and `plan` is hidden
- [x] 1.2 `agent-command`: candidates, annotations, insertion at the prompt start; harness tests from the prompt and from the transcript
- [x] 1.3 `agent-slash` bound to `/` in `agent-session-mode`: prompt at the prompt start with commands, else self-insert; harness tests

## 2. Review

- [x] 2.1 `/` at any word's start in the prompt area; `agent-command` inserts at point, or at the prompt's end from the transcript; harness tests
- [x] 2.2 Command chips: `Kind::Command` drawn ` ▷ /name `, sent as text, `RET` sends, replayed `/name`s chipped; chip and harness tests

## 3. Docs

- [x] 3.1 Record the behaviour in `docs/editor.md`'s ACP section
