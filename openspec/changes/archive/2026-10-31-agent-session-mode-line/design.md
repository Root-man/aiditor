## Context

The core already owns config options, turn state, pending permissions and exit state in Session. Buffer duplicates only the mode for the generic display builder; usage updates are not consumed. The existing ModeLine renderer provides separators, fitting and inactive faces.

## Goals / Non-Goals

Keep the session as the source of truth and use the existing renderer. No new widget, dependency, rate-limit display or config controls.

## Decisions

- Build a session ModeLine from Session at the frame boundary, replacing the generic bar and removing Buffer.agent_mode. This avoids duplicating turn, permission and config state across all lifecycle handlers.
- Retain only the latest usage pair and normalized title alongside session state. ACP 1.12 already exposes the typed usage variant without feature flags.
- Read display names from select options, flattening groups; fall back to the current id for unknown values. Retain raw mode ids in brackets to preserve S-TAB feedback.
- Defaults: omit idle status and unavailable fields; fall back to agent name for an unset title and plain face for unknown modes; floor context percentage, omit a zero-size update and cap at 100. Model and effort use their advertised capitalization, even where the issue example uses lower case.
- Use harness tests for core behavior and actual frame spans, plus recorded transcript replay assertions. The frontend draws the same spans as file and terminal bars.

## Risks / Trade-offs

- Future mode ids need risk mappings: unknown ids use the neutral face rather than guessing permissions.
- Existing fitting counts characters rather than rendered glyph widths; reuse its documented limitation.
- Browser-only computer use cannot drive the native editor: any unautomated rendering/key checks need user verification before proceeding beyond manual checks.
