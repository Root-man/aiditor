# Proposal

## Why

Auto-save (#50) and recovery on launch (#51) are tested only inside one process, with the harness standing in for the services. Nothing checks that a real process killed mid-edit leaves a recovery file that the next launch picks up, so a break between the core, the recovery service and the file service would go unnoticed. CI runners have no display, so the check can't go through the GUI (#53).

## What Changes

- A headless binary `aiditor-headless` in `aiditor-services`: the core plus the real file and recovery services, no GPUI. It takes a recovery dir, a file and a key script as arguments, opens the file, offers recovery files the way the app does at launch, feeds the script, carries out effects, and keeps firing the core's timers on the real clock until the core quits.
- An integration test that runs it twice on a temp dir: first typing an edit and waiting for its recovery file, then killing the process with SIGKILL; then relaunching with a script that accepts the recovery, saves and quits, and checking the file holds the edit and the recovery dir is empty.
- It runs under `cargo test`, so CI runs it on Ubuntu and macOS.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: this is test tooling; the recovery behavior it checks is already specified in `auto-save`. The change sets `skip_specs: true`.

## Impact

- New `crates/aiditor-services/src/bin/aiditor-headless.rs` and `crates/aiditor-services/tests/crash_recovery.rs`.
- No new dependencies. Unix only, like the recovery service it drives.
- `docs/editor.md` Crash safety notes the end-to-end test.
