# Design

## Context

The agent service runs `AgentOp::Spawn`, `Reply` and `Kill` and answers with `AgentStarted`, `AgentStderr`, `AgentRequest` and `AgentExited`; the core ignores all but requests. Terminals already solve the same lifetime problems: a `terminals: BTreeMap<TerminalId, BufferId>` map, `sync_terminals` in `Core::handle` sending a kill for every terminal whose buffer is gone (which covers `kill-buffer` and `workspace-kill`), `end_terminal` sending the one kill that releases the service's handle after an exit, `live_terminal` for the `kill-buffer` question and `!terminals.is_empty()` for the quit question. Project environments live in `Core::environments`, with `capturing` set while a capture runs and `vars` the last answer.

## Goals / Non-Goals

**Goals:** start, log and end sessions with the least new machinery, reusing the terminal patterns.

**Non-Goals:** the session buffer, prompts, modes (`default_mode` is not applied), permissions (requests stay refused), `additionalDirectories`, the root preamble, a session index that survives restarts, a key binding.

## Decisions

- **The log buffer is the session's handle.** A new `agents` module keeps `Core::agents: BTreeMap<AgentId, Session>`, each session holding its agent name, log buffer, project, the spawn held while the capture runs, the `session/new` answer once ready, whether it ended, and kept updates. A `sync_agents` pass beside `sync_terminals` drops sessions whose log buffer is gone and sends `AgentOp::Kill`, so `kill-buffer` and `workspace-kill` stop agents with no code of their own. Alternative: a session list tied to workspaces directly; it would need its own hook in `kill_workspace` and still a handle for `kill-buffer`. The session buffer later takes over the handle role.
- **Ending mirrors `end_terminal`.** On `AgentExited`, or a failed `AgentStarted`, the session is marked ended (it stops counting as live, but stays with its log buffer so late stderr still lands there) and one `AgentOp::Kill` sent: for an exited agent it only releases the service's handle (the service sends no signal once `exited` is set); for a failed start whose process lives on (an error answer to `session/new`) it stops it. Because `sync_agents` removes a session whose buffer was killed before its exit arrives, the crash note only appears for exits the editor didn't cause.
- **Waiting for the environment.** `agent-start` builds the spawn with the registry's and project file's `env`. If the project's environment has `capturing` set, the session holds that spawn; `environment_captured` then releases every waiting session of the project, prepending the captured vars. Otherwise the spawn goes out at once with `vars` (if any) prepended. Waiting on any running capture, re-captures included, follows the issue's wording and keeps one rule.
- **Live checks.** `kill-buffer` asks when `live_terminal` or the new `live_agent` holds for the buffer, and the quit question fires when a terminal or a session that hasn't ended is live.
- **Updates.** The service registers an `on_receive_notification` handler for `SessionNotification` and sends `Msg::AgentUpdate { id, update: Box<SessionNotification> }`; boxed to keep `Msg` small. The core pushes it on the session's `updates`.
- **Default agent.** `ArgSpec::preselect` picks the project's `default_agent` in the sorted registry names, so `RET` takes it; no new prompt machinery.

## Risks / Trade-offs

- [The log buffer can be killed by accident] → it asks first, as a live terminal does.
- [Stderr floods grow the log buffer unboundedly] → shortcut: accepted for now, agents write little to stderr; cap it if one floods.
- [A re-capture delays a new session by up to the capture timeout (5 s)] → acceptable; the echo area says it is starting.
