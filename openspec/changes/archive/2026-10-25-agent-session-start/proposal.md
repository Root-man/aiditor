# Proposal

## Why

The agent service (#279) can spawn an ACP agent and open a session, but nothing in the editor asks it to. This change gives the user a command that starts an agent session in the current workspace, with the project's environment, a place to read the agent's stderr, and a clean end when the agent crashes, the workspace is killed or the app quits (#280).

## What Changes

- New command `agent-start` (M-x): prompts `Agent: `, completing over the registry's agent names with the project's `default_agent` preselected, and starts that agent in the current workspace's project: `cwd` is the primary repo, the environment is the captured one, then the registry's `env`, then the project file's `env`.
- While the project's environment is being captured, the spawn waits and starts once the capture answers (or falls back).
- Each session gets a read-only stderr log buffer `*<agent> stderr*<<workspace>>`, owned by the workspace, that collects the agent's stderr lines. Until the session buffer exists, the log buffer is the session's handle: killing it stops the agent, asking first as for a live terminal.
- `*Messages*` notes `<agent> session ready (modes: ...)`, a failed start with its reason, and an exit after the session opened (a crash) with its code or signal. An exit ends only that session; the log buffer stays for reading.
- Killing a workspace kills its log buffers and so stops its agents. Quitting asks `Active processes exist; kill them and exit anyway? ` when an agent session is live, as it does for terminals.
- The agent service forwards `session/update` notifications to the core, which keeps them per session for the session buffer and renders nothing.
- The test agent grows an `update` behavior that sends one session update.

## Capabilities

### New Capabilities
- `agent-sessions`: starting an agent session from the editor, waiting for the environment, the stderr log buffer, `*Messages*` notes, ending sessions on crash, buffer kill and workspace kill, keeping session updates.

### Modified Capabilities
- `agent-processes`: session updates from the agent reach the core.
- `quitting`: the active-processes question also covers live agent sessions.

## Impact

- `aiditor-core`: a new `agents` module (sessions, the command, environment hold), `Msg::AgentUpdate`, quit and kill-buffer checks covering agent sessions, `Core::handle` reconciling killed log buffers.
- `aiditor-services/src/agents.rs`: a notification handler for `SessionNotification`; `aiditor-test-agent` gains the `update` behavior.
- `docs/editor.md`: records the command, the log buffer and the session lifetime under ACP integration.
- No frontend change: `Effect::Agent` is already run.
