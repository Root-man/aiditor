# Design

## Context

See proposal.md for motivation. `ProjectGrep` already tracks the request number, running flag, debounce deadline and retained hits. `Core::minibuffer` builds the display prompt; GPUI draws it and uses its width for text placement.

## Goals / Non-Goals

**Goals:** Reuse that lifecycle and the existing count without altering search scheduling or prompt identity.

**Non-Goals:** Search parallelism, caching, timeouts, new status faces and changing the retained-results behavior.

## Decisions

- Derive the running marker from the existing running flag, suppressing status while a debounce is pending. Track successful empty completion separately so errors never look like empty results. Clear it on query edits and new searches.
- Insert the marker before `Grep in <project>: ` in the display prompt only. Mutating the stored prompt would change its history key; a new frontend status field would duplicate existing prompt rendering and width handling.
- Preserve existing supersession timing: old results remain eligible until the next debounced search starts; while input is pending no previous-query status is displayed. Request-number checks continue to reject obsolete replies.

## Risks / Trade-offs

- Markers change the width when state changes: existing prompt measurement accounts for it, while the numeric count keeps its grow-only field.
- A blocked read can still delay hits: the running marker remains visible without promising progress or a timeout.
