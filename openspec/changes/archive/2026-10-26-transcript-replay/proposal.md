# Proposal

## Why

The committed Claude Code and Codex transcripts (#286) hold what the real adapters send, but nothing replays them, so the client is still only tested against messages we wrote ourselves (#287).

## What Changes

- A replay mode of `aiditor-fake-agent`: with `AIDITOR_FAKE_AGENT_REPLAY` naming a transcript, it answers each client message with the agent messages recorded after it, matching by method, remapping the client's request ids, putting the real `cwd` back in place of `{cwd}`, and exiting with an error that shows the message when the client sends one the transcript doesn't expect.
- A test that replays every committed transcript through the agent service into the core, typing each recorded prompt into the session buffer, and asserts every `session/update` reaches the core typed, every agent request is answered and every turn ends with its recorded stop reason; a second test shows an unexpected client message failing with the message shown.

## Capabilities

### New Capabilities

### Modified Capabilities
- `fake-agent`: a replay mode next to the script.
- `acp-transcripts`: committed transcripts replay against the client.

## Impact

- `crates/aiditor-services/src/bin/aiditor-fake-agent.rs`: the replay mode, plain JSON lines, no new dependencies.
- `crates/aiditor-services/tests/transcripts.rs`: the replay tests.
- `docs/editor.md` Testing and the transcripts README: how replay works.
