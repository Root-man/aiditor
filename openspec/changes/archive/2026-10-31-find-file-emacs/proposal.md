# Proposal

## Why

Issue #178 asks for Emacs-style file completion; review extends the same behavior to project-find-file. Automatically selecting the first entry obscures typed-path acceptance, and bare names lack the metadata and directory styling in approved frames 4 and 12.

## What Changes

- Show `*/N` with no selected candidate for an empty query in find-file and project-find-file; navigation selects a candidate and RET with no selection uses the typed path.
- Draw filesystem directories in the existing `directory` face.
- Annotate filesystem entries with Unix mode bits, decimal human-readable size and compact modification age, in dimmed columns at the right edge, outside fuzzy matching.
- Keep non-file completion and directory-only prompt selection behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-visiting`: find-file selection and annotated filesystem entries.
- `minibuffer`: filesystem candidate styling, annotation layout and unselected count.
- `file-finder`: unselected empty project queries and asynchronous metadata for visible indexed files.

## Impact

File-service listings and visible-file metadata replies, plus core harness fixtures; minibuffer selection and display; GPUI completion rendering and tests. No new dependencies, persistence or filesystem writes.
