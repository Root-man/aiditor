# Design

## Context

#422's locally verified project-row snapshot is the prerequisite; its active delta is authoritative until the stack is archived. Existing buffers already support named text properties and an elastic AlignRight space. Existing mode lines own path formatting and session model/effort/activity semantics.

## Goals / Non-Goals

Goals: build metadata once per refresh, reuse existing display/property machinery, and keep normal buffer movement/scrolling intact. Non-goals: folding, echo hint changes, live refresh, narrow-window truncation, new faces or persistence.

## Decisions

- Build escaped name/where/state pieces into the ordinary read-only snapshot and attach named face properties. Keep where/state at the right through AlignRight, with minimum character widths corresponding to frame 13's 420px/240px columns and its column gap; longer values keep their text. This avoids introducing a second renderer. #426 owns fitting/truncation at narrow widths.
- Reuse file/terminal mode-line output for paths and terminal state; expose session model/effort text from its existing computation. Share agent activity words and workspace urgency with the workspace modeline.
- Count the same owned-buffer inventory that the list displays; unsaved means modified file buffers, consistent with save prompts. File metadata uses its owning project's roots even when another workspace is active.
- Use ordinary named face properties so changing theme recolors without rebuilding the snapshot. Store selectable name columns alongside row identity. Apply completion-selected dynamically at redisplay so moving point needs no snapshot rebuild.
- Keep all original control-character escaping. No raw external string may create a new logical row.

## Risks / Trade-offs

- Long values and narrow windows still wrap using ordinary buffer layout; #426 supplies truncation.
- Character sizing follows the existing monospace column machinery; no new glyph measurement is introduced.
- Native screenshots and Linux runtime checks are unavailable in this environment; core display assertions cover row text, point, and theme-resolved faces.
