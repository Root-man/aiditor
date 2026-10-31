## Why

Issue #359 replaces the generic buffer mode line in agent sessions with the session information needed to judge what the agent is doing and how much context it has used.

## What Changes

- Show model and effort display names, the risk-coloured mode, and the session title on the left.
- Show working, awaiting-permission or exited status, and context percentage from ACP usage updates on the right.
- Follow session configuration changes, mode cycling, titles and lifecycle events; keep existing fitting and inactive-window styling.
- Document the session mode line in docs/editor.md and cover both recorded adapters.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: Replace the mode-only bar with session metadata, lifecycle status and context usage.
- `display-model`: Extend the state-face rules to session mode lines.

## Impact

Core session state, buffer display metadata and mode-line construction; ACP schema usage support if feature-gated; core and transcript replay tests. No new dependency or rate-limit display.
