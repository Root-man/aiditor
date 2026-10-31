# Design

## Context

Grep owns hits and their full labels; the filtering minibuffer selects only hit labels. Its display and the GPUI frontend currently render those labels directly. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:** Keep navigation, preview and acceptance on hit identities while introducing file headers in the display. Bound the viewport to ten physical rows, headers included.

**Non-Goals:** Changes to the grep service, new settings, query syntax or additional highlighting.

## Decisions

- Keep full hit labels as command identities. Regroup the hit list by `(root, path)` in first-seen order after batches, using stable ordering within each file. Preserve selected and top hit identities when replacing the completing prompt's items. The user selected this behavior over repeating headers in global arrival order.
- Add header metadata to the minibuffer display, separate from candidates. Candidates for grep display contain only the line number and text. The frontend draws headers on rules using `grep-file` and `separator`, and numbers with `grep-line-number`. Other completion lists keep their existing rendering.
- Include headers in the ten-row viewport and grow-only height. Repeat the visible file's header when the viewport starts inside its group; shift the top only enough to keep the selected hit visible. Streaming insertion may change rows within a group, but retains the selected and top hit where they still fit.
- Show root names before relative paths when the search covers multiple roots; omit them for a single-root search, as the supplied frame 3 export does. Keep full labels internally in either case.

- Keep grep status separate from the prompt string. Reserve the longest status plus two columns for the entire query prompt, and draw status at the right inset. This keeps both input origin and horizontal scrolling stable across status transitions, including long queries. Root picking and other prompts reserve no status space.

## Risks / Trade-offs

- Regrouping can insert rows above a selected hit → preserve its identity and preview and fit the viewport again.
- Headers consume space → count them within the existing row limit, keeping each visible header with at least one hit.
- Similar file names in different roots → group by root and path, displaying unique finder root names.
- Paths and line text may contain colons → derive display directly from stored hits, never parse full labels to find file identity.

## Migration Plan

No persistent data or service protocol changes. Reverting the commit restores the flat display.
