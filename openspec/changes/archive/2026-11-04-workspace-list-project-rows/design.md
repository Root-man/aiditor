# Design

## Context

The list stores `(line, WorkspaceId)` targets on its buffer. Its generic fileless mode line shows a read-only marker and text coordinates. Existing `Core::switch_project` already resolves identity, most recently shown workspace and first-workspace creation.

## Goals / Non-Goals

**Goals:** Share one typed row inventory between navigation, activation and mode-line counts, retaining snapshot behavior.

**Non-Goals:** Replacing the ordinary text display, folding, styling, destructive actions or live updates.

## Decisions

- Replace workspace-only targets with a small Project/Workspace enum; keep physical text line offsets so ordinary text movement continues to work.
- Preserve populated project order from workspace creation order; append registered projects absent from that inventory using stable name sorting. Always render roots, eliminating duplicate-name branching.
- Validate targets before dismissing the list, then delegate project activation to `switch_project` and workspace activation to `switch_workspace`.
- Derive counts and row position from the snapshot targets in a specialized fileless mode line, avoiding redundant state. Informational-line positions use the preceding selectable row, clamped to the first.
- Keep the existing requirement title for navigation so the delta modifies the existing contract without creating competing requirements. Keep the delta pending until archive syncs the main spec.

## Risks / Trade-offs

- Snapshot entries can outlive their targets → validate identity before dismissing; existing refresh behavior rebuilds the inventory.
- Empty retained ad-hoc projects now appear → intentional: every registered project is listed, with no new lifecycle behavior.
- Real native drawing/input is outside the core harness → automate all new behavior and frame mode-line output; report native UI coverage separately.
