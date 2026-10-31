# Proposal

## Why

Project grep can spend seconds searching before finding its first hit, leaving users unable to distinguish work in progress from an empty result. Issue #338 asks for visible search progress and an explicit empty result.

## What Changes

- Show `[searching]` in the grep prompt during the current search, alongside the existing growing count.
- Show `[no matches]` after a successful search finishes empty.
- Clear obsolete status when the query changes or the prompt closes; preserve debounce, retained hits, history and regexp error reporting.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: make the active search lifecycle visible in its prompt.

## Impact

Core grep state and minibuffer display, core harness and GPUI tests, and `docs/editor.md`. No new dependencies or service protocol changes.
