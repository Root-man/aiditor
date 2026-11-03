# Design

## Context

See proposal.md for motivation. The core already has a project finder with captured root labels and asynchronous first-use filling, and the index retains absolute directories for watch coverage. Chips already handle display, motion, undo and ACP serialization. The minibuffer restores the origin window and its live point marker when it closes.

## Goals / Non-Goals

**Goals:** Reuse those paths for the reference picker without walking the disk during typing or duplicating chip behavior.

**Non-Goals:** General filesystem attachments (#371), new chip types or protocol changes, changes to project-find-file's previews and file-only candidates.

## Decisions

- Bind `@` to `agent-reference` in session mode. Insert it before opening the picker so cancellation naturally preserves it. Every typed `@` in the editable prompt triggers, as the issue says; bulk pasted text keeps the ordinary insertion path.
- Share the project finder prompt setup with a reference mode. That mode includes indexed directories (including empty ones) with a trailing slash and root-qualified labels, requires a selected match, and has no preview. Reuse the existing directory face and asynchronous file annotations, showing only the age field for files and `dir` for directories, as frame 6b does. Keep project-find-file's behavior unchanged.
- Use the picker's captured mapping to resolve the selected path before closing. Keep the trailing slash when resolving directory candidates so the handler can select the chip kind without a disk read.
- Replace the char immediately before the restored point, checking it is `@` in the editable prompt. The origin point is a buffer marker, so streamed output moves the location correctly; a raw offset in command arguments would become stale. Intern and insert the existing chip; no added separator or trailing space.
- Keep an open picker's candidates and mapping stable across project reloads; only a waiting picker accepts an index reply with the same roots, as the project finder already does.

## Risks / Trade-offs

- Agent output moves the prompt during completion → test selection and cancellation while streaming.
- Duplicate root basenames can resolve incorrectly → reuse and test captured root labels.
- Shared finder changes could affect ordinary file finding → retain its file-only candidates and run its tests and workspace checks.
- Existing chip limits (URI encoding, clipboard export and external-root rechecks) remain governed by the chip implementation; this picker only offers project-index paths.
