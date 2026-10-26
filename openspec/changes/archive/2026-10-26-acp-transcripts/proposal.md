# Proposal

## Why

The ACP client is tested against the fake agent only, which sends what we think the adapters send. Real traffic from `claude-agent-acp` and `codex-acp`, recorded with aiditor's own client capabilities, lets later tests replay what the agents really do (#286).

## What Changes

- New binary `aiditor-acp-record OUT -- COMMAND [ARGS...]` in `aiditor-services`: runs the agent, passes stdin and stdout through unchanged, and appends every JSON-RPC line to OUT as a JSON line with its direction. It can sit in front of any agent with a registry entry.
- An ignored test that records a short scripted session with each adapter through aiditor's agent service (the same `initialize`, `session/new` and prompts the editor sends, and the core's refusal of agent requests), then scrubs the home directory and the session `cwd` into placeholders.
- Scrubbed Claude Code and Codex transcripts in `crates/aiditor-services/tests/transcripts/`, with a README on recording and scrubbing, and a test that the committed transcripts parse and hold no home path.

## Capabilities

### New Capabilities
- `acp-transcripts`: the recorder binary and the committed, scrubbed agent transcripts.

### Modified Capabilities

## Impact

- `crates/aiditor-services`: new `src/bin/aiditor-acp-record.rs`, new `tests/transcripts.rs` and `tests/transcripts/`. No new dependencies.
- `docs/editor.md` Testing: a short section on the recorder and transcripts.
- Recording uses the developer's own agent logins; it never runs on CI.
