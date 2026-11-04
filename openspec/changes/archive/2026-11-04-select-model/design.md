# Design

## Context

See proposal.md. `agent-cycle-mode` already finds the mode option (`mode_option`), flattens a select's values (`options`), sends `AgentOp::SetConfigOption` and records the value sent in `mode_sent` for a refusal's note. `agent_configured` stores the answer's options, which the mode line reads.

## Goals / Non-Goals

**Goals:** Reuse the cycle-mode path for sending, answering and refusals; check readiness before any prompt opens.

**Non-Goals:** A key binding, effort selection (a sibling issue), tracking values in flight.

## Decisions

- The command declares no prompts and asks its own question with `read_string` and the model names as choices, so the not-ready and no-model checks run before the minibuffer opens. `spec.annotation` and `spec.preselect` still apply to a handler's choices, giving descriptions and the current value.
- Generalise the category lookup: `select_option(options, category)` serves both the mode and the model.
- `mode_sent` becomes `sent`, holding `mode <value>` or `model <value>`, so the refusal note reads `<agent> could not set <sent>: <reason>` for both.
- The answer maps a name back to the first value with that name; typed text naming no value echoes `<agent> has no model <text>`.
- Messages reuse `S-TAB`'s wording (`<agent> is not ready`, `<agent> session has ended`) and add `<agent> has no model choice`; the issue's `Session not ready` is replaced by the existing message for consistency.
- The fake agent keeps its current mode and model in a static, so setting one keeps the other.

## Risks / Trade-offs

- Two values sharing a display name → only the first is reachable; no adapter does this today.
