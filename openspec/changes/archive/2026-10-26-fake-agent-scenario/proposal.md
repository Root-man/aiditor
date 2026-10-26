# Proposal

## Why

The agent, file and watcher services are each tested alone, but nothing checks that an agent writing a file the user has open reaches that buffer through the real services, or that a whole workspace's teardown stops its agent. The testing plan in `docs/editor.md` calls for whole-app fake-agent scenarios once prompting exists (#292, done).

## What Changes

- A new integration test, `crates/aiditor-services/tests/scenario.rs`, runs the core with the real agent service, file service and native watcher on a temporary project whose settings register the fake agent.
- A key script runs `agent-start` and sends a prompt; the fake's script streams a reply and writes a file the project has open.
- It asserts the reply reaches the session buffer, the open, clean buffer reverts to the new contents, and killing the workspace leaves no agent process.
- `docs/editor.md`'s Testing section names the scenario.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. No behavior changes; the change only adds a test (`skip_specs: true`).

## Impact

Test code in `aiditor-services` and the Testing section of `docs/editor.md`. No production code, dependencies or APIs change. Runs under `cargo test` on macOS and Linux.
