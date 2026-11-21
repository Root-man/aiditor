# Design

## Old side as a hidden buffer

The old text goes into a buffer named ` *diff old*` (a leading space hides it from buffer lists) given the file's grammar. That reuses everything a file buffer has: the parse on creation under `PARSE_CUT`, the `Parse` effect asked by `ask_parses` over it, and `Msg::Parsed` routed by buffer id, so a late parse for an earlier file can't land on the current one. It is killed with the shown diff, like the index side buffer.

## Faces resolved when rows are built

Diff rows hold `FaceRef`s resolved at display time, but `Syntax::highlights` answers resolved `Face`s for a palette. A new `FaceRef::Is(Face)` carries them, on top of `diff-removed`. Since they are resolved early, the rows are rebuilt when the old buffer's parse arrives and when settings load. Highlighting all removed lines at once is one `highlights` call over their ranges; the parse keeps multi-line context, so a removed line inside an old comment or string is coloured as such.

## Alternatives

- Highlight in `frame` from the old buffer, only the visible rows: needs the row's old line threaded through `DiffRow` and `splice_diff_rows`; more plumbing for a cost that only shows on huge deletions.
- Keep a `Syntax` in `Shown`: needs its own parse routing and a guard against a stale parse for another file.
