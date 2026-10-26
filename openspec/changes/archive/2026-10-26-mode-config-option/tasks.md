# Tasks

## 1. Agent service

- [x] 1.1 Add `AgentOp::SetConfigOption` and `Msg::AgentConfigured` in `aiditor-core/src/lib.rs` and send the request in `aiditor-services/src/agents.rs` like `Prompt`; verify with a test in `tests/agents.rs` setting the fake's `mode` to `plan` (answer has `plan`) and to `nope` (error)
- [x] 1.2 Make the fake agent advertise the `mode` select option instead of modes, answer `session/set_config_option` and send `config_option_update`, dropping `session/set_mode`; update `tests/fake_agent.rs` (`setting_the_mode_reports_it`, start test) and verify they pass

## 2. Core

- [x] 2.1 Read the mode option in `agent_started` (ready note, `agent_mode`) and in `agent_update` (`ConfigOptionUpdate` instead of `CurrentModeUpdate`); update the existing core tests and verify they pass
- [x] 2.2 Apply `default_mode` after `session/new`: send `SetConfigOption` when it is a value and not current, note an unknown one; handle `AgentConfigured` (update `agent_mode`, note errors); verify with core tests for each scenario of the agent-sessions delta
- [x] 2.3 Check the transcript replay and the whole-app scenario still pass (`cargo test -p aiditor-services`)

## 3. Docs

- [x] 3.1 Record config options over modes, and why, in `docs/editor.md` (Modes paragraph, agent registry and process notes, fake agent section); verify by reading the diff
