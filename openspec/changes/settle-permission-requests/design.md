## Context

`Ctx::agent_request` shows a permission request from a ready session and refuses everything else with "method not found". `agent_cancel` and `agent_exited` already settle pending requests (#307). Killing a session buffer makes `sync_agents` forget the session and send `AgentOp::Kill`; the service can still deliver requests the agent sent before it died.

## Goals / Non-Goals

**Goals:** no permission request is left unanswered or answered with a protocol error once the session has ended or been forgotten.

**Non-Goals:** cancelling requests that arrive between `session/cancel` and the turn's end (they show and wait, the agent still lives); changing refusals before `session/new` is answered.

## Decisions

- One match in `agent_request`: a permission request from a ready session is shown; from an ended or unknown session it gets the `cancelled` outcome; anything else is refused. An unknown id can only be a forgotten (killed) session, since ids come from spawns.
- Before ready stays "method not found": no session id exists yet for the agent to have asked about, and #307's spec keeps it.

## Risks / Trade-offs

- An agent that asks after `C-c C-c` but before answering the prompt gets a block that waits; ACP only requires cancelling requests pending at the cancel.
