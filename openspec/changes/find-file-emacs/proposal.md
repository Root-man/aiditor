# Proposal

## Why

Issue #178 asks for an Emacs-style find-file list. Automatically selecting the first entry obscures typed-path acceptance, and bare names lack the metadata and directory styling in approved frames 4 and 12.

## What Changes

- Show `*/N` with no selected candidate for an empty find-file filename query; navigation selects a candidate and RET with no selection uses the typed path.
- Draw filesystem directories in the existing `directory` face.
- Annotate directory entries with Unix mode bits, decimal human-readable size and compact modification age, in dimmed columns at the right edge, outside fuzzy matching.
- Keep non-file completion and directory-only prompt selection behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-visiting`: find-file selection and annotated filesystem entries.
- `minibuffer`: filesystem candidate styling, annotation layout and unselected count.

## Impact

File-service listings and the core's listing reply/harness fixtures; minibuffer selection and display; GPUI completion rendering and tests. No new dependencies, persistence or filesystem writes.
