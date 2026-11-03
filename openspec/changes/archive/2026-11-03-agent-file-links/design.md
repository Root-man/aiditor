# Design

## Context

The core does no disk I/O; the frontend and services do. What exists under a project's roots is already known in the core through the project file index (`FileIndex::roots`: each root with its sorted relative file paths), which `find-file` and file watching keep current. Agent messages are rendered through `markdown::render`, and tool headers through `render_tool`, both as text properties on the session buffer.

## Goals / Non-Goals

**Goals:** links where the issue asks for them, checked against the index, and `RET` to visit.

**Non-Goals:** links in thoughts, the echoed prompt or tool bodies; markdown link destinations (`[x](src/a.rs)`); mouse clicks; re-rendering old blocks when the index changes.

## Decisions

- **Existence is the index, not the disk.** A file is linkable when the project's index lists it. This keeps the core pure and costs a binary search per token. Ignored files (e.g. under `target/`) and files the index hasn't seen yet are not links. Alternative: a filesystem check per token from the core, rejected as I/O on the UI thread.
- **Links are faces, found again on `RET`.** `markdown::render` takes an `is_file` test and adds `link` + underline faces over path tokens that pass, as one more face layer in its existing piece algorithm, so props keep ascending. On `RET`, the line under point is tokenized again and the token under point resolved, if point carries the `link` face. No new `Prop` variant; the face is the marker.
- **Tool headers store their link.** `show_tool` resolves the header's path against the index and keeps `(path, line)` on the `Tool`; `render_tool` faces the target range when it is set; `RET` on that range visits it, elsewhere on the header opens the output as before.
- **Location lines are 1-based**, as Claude's adapter sends `line: 1` for a read from the top. A `path:line` reference is 1-based as editors print them.
- **Visiting** reuses project grep's visit-at-line code (find-file, then jump now or once read), extracted into one helper, run in the other window the way `switch-to-buffer-other-window` picks it.

## Risks / Trade-offs

- [Links are fixed when a block renders] → a file created after the message isn't linked until the block re-renders; acceptable, noted as a shortcut.
- [Paths in code blocks link too, and a markdown link's destination gets the face but stays hidden] → wanted for code blocks; harmless for destinations.
- [`RET` resolves the word against the current index] → a file dropped from the index after rendering still looks linked but `RET` sends the prompt; rare.
- [A word equal to a root-level file name, e.g. `Makefile`, links] → harmless and often wanted.
