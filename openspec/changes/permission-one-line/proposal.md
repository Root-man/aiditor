# Proposal

## Why

An answered permission request leaves `Chose Yes (allow_once) for Edit a.txt`, which repeats the ACP kind and the title, and a request still open when its turn ends stays open with nothing left to answer it (#366).

## What Changes

- An answered request's block becomes one dim line from the option's kind: `✓ allowed once`, `✓ allowed always`, `✗ rejected` or `✗ rejected always`.
- A cancelled or dropped request's block becomes `✗ cancelled`.
- When the prompt's answer ends a turn with requests pending, each is answered `cancelled`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: the lines answered and cancelled requests leave, and cancelling when a turn ends.

## Impact

`crates/aiditor-core/src/agents.rs`, `agents/render.rs`, the tests in `aiditor-services`, `docs/editor.md`.
