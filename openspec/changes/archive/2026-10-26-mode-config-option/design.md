# Design

## Context

`agents.rs` reads `NewSessionResponse::modes` for the ready note and the buffer's `agent_mode`, and follows `CurrentModeUpdate`. The agent service has ops for spawn, reply, prompt, cancel and kill; the prompt op shows the pattern for a request whose answer goes back to the core as a `Msg`. `agent-client-protocol-schema` 1.12 has config options in stable v1 (`SessionConfigOption`, `SetSessionConfigOptionRequest`, `ConfigOptionUpdate`).

## Goals / Non-Goals

**Goals:** read the mode from config options; apply `default_mode` once at start.

**Non-Goals:** a key to cycle or pick modes, other config options (model, thought level), boolean options, showing option names instead of values.

## Decisions

- **One helper finds the mode option.** `fn mode_option(&[SessionConfigOption]) -> Option<(&SessionConfigId, &SessionConfigSelect)>`: the first option with category `Mode` and kind `Select`. A mode-category boolean is not a mode. Values are flattened from `Ungrouped` or `Grouped`. The ready note, the default-mode check and every update use it.
- **The answer to `set_config_option` comes back as `Msg::AgentConfigured { id, result: Result<Vec<SessionConfigOption>, String> }`.** ACP returns the full option list in the answer, and an adapter may not also send `config_option_update`; reading the answer makes the mode line correct either way. An error is noted in `*Messages*`. Alternative: drop the answer and rely on the notification — simpler, but leaves the mode line stale with an adapter that only answers, and hides errors.
- **`AgentOp::SetConfigOption { id, request: SetSessionConfigOptionRequest }`**, mirroring `Prompt` (the core builds the request with the session id).
- **The fake drops session modes and `session/set_mode`.** Nothing reads them anymore; keeping them would be dead code.
- **Messages.** Unknown: `<agent> has no mode <value>`; refused: `<agent> could not set mode <value>: <reason>`. Neither is echoed: the ready note already is, and these come right after it.

## Risks / Trade-offs

- [An adapter that sends modes but no mode config option shows no mode] → both supported adapters send one (recorded transcripts); docs record it.
- [Codex's adapter may answer `set_config_option` and also send `config_option_update`] → both set the same value; harmless.
