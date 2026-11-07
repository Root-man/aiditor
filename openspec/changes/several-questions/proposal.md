# Proposal

## Why

claude-agent-acp sends an `AskUserQuestion` with several questions as one form: a `question_<n>` choice field per question, titled with its header and described with the question, each with its own `question_<n>_custom`; codex-acp sends one field per question with a note field naming its `questionId`. Today such a form is declined. Issue #409 draws it as frame 6e.

## What Changes

- A form whose properties are several choice fields, each with at most one companion, is a question with several fields, shown as one `Questions <k> of <n>` block: one field open with its options, the others folded to a line marked `✓` with their answer once answered and `○` until then, named by their `title`.
- `TAB` with point in the block opens the next field. A single-choice field's answer (digit, `RET` or own answer) records it and opens the next field; a multi-select field ticks as before.
- `C-c C-y` with point in the block sends every answer so far, leaving unanswered fields out; `C-c C-k` skips the whole block.
- Companions pair with their choice by `<key>_custom` or a codex note's `_meta.codex.questionId`; fields show in key order, numeric suffixes compared as numbers.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-questions`: several questions in one block.
- `session-buffers`: `TAB` and `C-c C-y` in a block of several questions.

## Impact

- `crates/aiditor-core/src/agents/questions.rs`, `agents.rs`.
- `docs/editor.md`.
- No new dependencies.
