# Design

## Context

The core receives only names in Listed replies, nucleo matches names alone, and completion selection is currently an index. Existing annotations and theme faces can be reused. See proposal.md for motivation; supplied frames 4 and 12 define layout and faces.

## Goals / Non-Goals

**Goals:** Keep listing IO on the existing worker, preserve request invalidation and fuzzy highlighting, and reuse the existing renderer and faces.

**Non-Goals:** A theme overhaul, live age timers, changing directory-only prompt selection, and changing list ordering or row count.

## Decisions

- Carry a small directory entry with name and preformatted annotation in Listed. Format metadata on the file worker rather than introducing filesystem/time access in the core. Harness fixtures can provide the same entries with blank annotations.
- Represent a missing selection explicitly in completion, while ordinary prompts continue selecting index zero. Empty-query find-file and project-find-file clear selection; nonempty typing keeps existing first-match behavior. TAB uses the first match when unselected; RET keeps typed-path acceptance. C-p from no selection picks the last match.
- Use symlink_metadata for modes and modification time; preserve target-following directory classification. Unix mode formatting covers special permission bits and file types. Failed fields use an em dash. Decimal size uses one fractional digit unless integral; age uses the two largest adjacent units (days/hours, hours/minutes, minutes/seconds), flooring and clamping future timestamps to zero.
- For project-find-file, request metadata only for the at-most-ten visible matches on the existing blocking file worker. Cache requested names and returned annotations for that prompt session; revisiting a row reuses its snapshot. Include the prompt session in replies so a closed/reopened finder cannot accept old metadata. Resolve paths through the finder's captured root mapping, preserving duplicate root labels and reload behavior. The index remains path-only.
- Mark both file finders as filesystem lists in the display model. Use that marker to apply directory faces and right-align preformatted metadata. Existing non-filesystem annotations stay beside names. Clip long names with flex sizing so metadata columns hold position.

## Risks / Trade-offs

- Extra stat calls per entry increase listing cost → use the existing blocking listing worker, never stat during rendering.
- Metadata changes after listing → snapshot semantics; reopening/relisting refreshes it.
- Narrow frames cannot fit all metadata → preserve right alignment and clip at row bounds.
- Native UI automation is unavailable in this session → automate frontend behavior through GPUI and request user hand checks for the drawn result before shipping.
