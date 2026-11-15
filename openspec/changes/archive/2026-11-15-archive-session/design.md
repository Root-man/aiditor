## Context

`*Sessions*` lists each agent through a throwaway process (`AgentOp::List`). Rollback already deletes a session through `AgentOp::Delete`, which needs a running agent of the same kind; the sessions list usually has none. codex-acp 2.6.0's `session/delete` is `thread/archive`.

## Goals / Non-Goals

**Goals:** `a` archives a Codex session; other agents fail with `Not supported yet`; an open session is stopped first.

**Non-Goals:** `session/archive` and archived listings (#481), the Archived group's content, `D` delete.

## Decisions

- **Delete inside the list request.** `AgentOp::List` gains `delete: Option<(SessionId, Option<AgentId>)>`: after `initialize` the throwaway sends `session/delete` of the session, then lists as before, answering `Msg::AgentListed`. The fresh answer is what drops the row, so no new message is needed, and the held-until-environment path for list requests works unchanged. A new `Archive` op would duplicate the list's spawn, cleanup and quit handling.
- **Which agent archives.** Known by its registry name `codex`, as `agent_look` knows glyphs; the adapter's capabilities can't tell, since claude-agent-acp also advertises `session/delete`, which really deletes. `shortcut:` until #481.
- **Stopping an open session.** The core finds the open session's agent and puts it in the request; the service removes its handle and terminates it as `Kill` does, and the delete waits until its process group is gone, as `AgentOp::Delete` waits (Claude writes its transcript on exit; Codex may flush its rollout). The buffer stays and ends through the normal exit message: the issue asks for the process stopped, and killing the buffer would drop an unsent prompt.
- **Immediate feedback.** The core calls `forget_listed` so the row leaves every list of the project at once; the listing's request number is bumped so only the fresh answer lands.
- **Failure.** A delete the agent can't do or that errors fails the whole list answer (`codex: archiving <id>: <reason>`), shown as the agent's error line; `g` lists again.

## Risks / Trade-offs

- A failed archive hides the agent's rows until `g`. Accepted: rare, and the reason is shown.
- Two quick archives run as two throwaway processes; the second list may answer before the first delete lands, so the first row can come back until `g`. A second `a` while the environment is still being captured replaces the first held request, whose session then stays unarchived. Accepted: rows only exist once a list ran, so the environment is captured.
- An agent registered under another name than `codex` that runs codex-acp can't archive. Accepted until #481.
