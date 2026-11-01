# Proposal

## Why

Swiper's `C-s` result list loses source syntax colours even though its preview window is highlighted. M5 task #402 closes this gap with the source colours and font styles users already see while editing.

## What Changes

- Preserve syntax faces in visible Swiper result text, including multiline context, injected languages and local references.
- Keep source text and syntax snapshots aligned throughout a prompt, including source edits or reverts.
- Apply the current theme on redraw while preserving plain line-number prefixes and the selected-row background.
- Cover the core display data and frontend rendering with automated tests and document the behaviour.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `line-search`: Swiper results retain syntax faces from their source snapshot.

## Impact

The core syntax snapshot, Swiper, minibuffer display model, frontend completion rendering, tests and editor documentation change. Existing search matching, preview and acceptance semantics remain. No dependencies, configuration options or project-grep syntax colouring are added.
