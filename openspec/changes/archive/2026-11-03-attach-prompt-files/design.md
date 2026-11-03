# Design

## Context

See proposal.md for motivation. Filesystem prompts already support asynchronous directory listings, metadata, completion and directory deletion. Their command name is a minibuffer minor mode. Session chips already encode paths and generate ACP content.

## Goals / Non-Goals

**Goals:** Reuse the filesystem picker and chip machinery across command registration, minibuffer acceptance and session insertion.

**Non-Goals:** New attachment rendering, ACP payloads, image decoding, global shortcut, or new filesystem validation semantics.

## Decisions

- The command asks its own question after checking it is in a session, like project references. Use `files: Some(0)` and `unselected: true`, with no preview. Default to find-file's directory rather than adding an attachment-specific location.
- Add a directory-entry choice to the existing minibuffer acceptance path. The existing RET and M-RET callers retain their behavior; the attachment picker's C-RET accepts the selected candidate without entering it. Avoid a new configurable prompt option or a second acceptance implementation.
- Intern the accepted absolute path in the existing chip table, recognizing directories by the filesystem candidate's trailing slash. Typed paths follow existing file prompt behavior; users can include a trailing slash to name a directory. Missing paths retain existing missing-chip behavior.
- Insert one chip with no added whitespace at prompt point. From transcript point, append at prompt end, following the existing command's destination convention. The saved origin window already tracks streamed insertions, so no extra marker is needed.

## Risks / Trade-offs

- [Changing shared minibuffer acceptance could affect find-file] -> Keep existing callers' directory navigation and exercise the existing tests.
- [Outside-root paths are not watched] -> Retain the existing chip limitation documented in editor.md; this change does not add watches.
- [A directory named without a trailing slash through typed-input acceptance is treated as a file] -> Normal candidate acceptance supplies the slash; current-directory attachment uses the prompt's slash. No synchronous disk calls in core.

## Migration Plan

No persistent format changes. Reverting the command and its picker binding removes the feature.
