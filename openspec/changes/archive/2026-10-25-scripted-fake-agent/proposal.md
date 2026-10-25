# Proposal

## Why

The agent service tests drive `aiditor-test-agent` through a handful of hard-coded `AIDITOR_TEST_AGENT` behaviors, so every new agent scenario (streaming, permissions, files on disk, turns, modes, cancel) needs new Rust in the fake. The coming session buffer, permission and whole-app work all need an agent that can play arbitrary ACP conversations without code changes (#285).

## What Changes

- **BREAKING (tests only)**: `aiditor-test-agent` becomes `aiditor-fake-agent`, and `AIDITOR_TEST_AGENT` behaviors (`request`, `update`, `exit3`, `trap`) are removed.
- `AIDITOR_FAKE_AGENT` names a JSON script file: steps run after `session/new`, and one list of steps per `session/prompt` turn.
- Steps: send a `session/update` written as ACP JSON, ask a permission and wait for the answer, write a file relative to the session's `cwd`, sleep, write a line to stderr, exit with a code, end the turn with a stop reason, ignore SIGTERM.
- The fake answers `session/set_mode` and sends a `current_mode_update`, and ends the running turn with `cancelled` on `session/cancel`.
- The agent service tests are ported to scripts; a new test drives the fake directly as an ACP client through a prompt turn.
- The script format is recorded in `docs/editor.md` under Testing.

## Capabilities

### New Capabilities
- `fake-agent`: the scripted ACP agent used by tests: script format, steps, prompts, modes and cancel.

### Modified Capabilities
None. The agent service's requirements are unchanged; only the tests behind them move to scripts.

## Impact

- `crates/aiditor-services/src/bin/` (the fake), `crates/aiditor-services/tests/agents.rs`, a new `crates/aiditor-services/tests/fake_agent.rs`.
- `docs/editor.md` Testing section.
- No new dependencies (`serde`, `serde_json`, `tokio`, `tokio-util` are already there).
