# Proposal

## Why

Completion lists show which candidates match, but not why. Issue #201 makes the matched characters visible in every fuzzy completing prompt.

## What Changes

- Draw the characters matched by nucleo in the existing `completion-match` face, including in `M-x`, buffer switching, project files and directory-based file finding.
- Preserve selection backgrounds and dimmed annotations.
- Handle Unicode candidates, multi-atom queries and changes to the query or visible rows using the displayed match snapshot.
- Keep Swiper and project grep unchanged, as confirmed by the user.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `minibuffer`: fuzzy completion candidates expose and draw matched character spans.

## Impact

The shared core completion display, its public display model, the GPUI completion renderer, their tests and `docs/editor.md`. Reuse the already installed Unicode segmentation library as a direct core dependency. No new settings or matching behavior.
