# Design

## Context

The list is rebuilt from the git answer by `render_diff_list` in `crates/aiditor-core/src/diff.rs`, which also records `rows: Vec<(line, Row)>` (`Row::Root`, `Row::File`). Folded roots live in `DiffList::folded: BTreeSet<PathBuf>`, toggled by `diff_toggle`, which finds the nearest `Row::Root` at or above point. Group headings are plain text today (`    Staged  1`) with no row entry. `REDIFF` is the debounce timer's duration.

## Goals / Non-Goals

**Goals:** the three behaviours in specs/diff-view/spec.md, with the least change to the list's code.

**Non-Goals:** folding or counts in the turn diff (frame 15c has no groups); new keys; a setting for the debounce.

## Decisions

- **Group rows.** Add `Row::Group { root, staged }` and record it for each group heading, so `diff_toggle`, refresh's point-keeping and `fit` see headings without parsing text. `fit` already matches only `Row::File`, so headings are never cut; the motion commands filter on `Row::File` too.
- **One folded set.** Change `folded` to `BTreeSet<(PathBuf, Option<bool>)>`: `None` the root, `Some(staged)` a group. Simpler than a second set, and a group's fold survives its root's fold for free.
- **Toggle target.** `diff_toggle` takes the nearest `Row::Root` or `Row::Group` at or above point. A file row is always under its group's heading, so that lands on the group; a root heading lands on the root. The "nothing to fold" check stays for a root without files; a group always has files.
- **Heading layout.** `  ▾ Staged     1`: two spaces, mark, the name padded to 7 (`Changes`), two spaces, the count `{:>3}`. The mark sits where the indent was, so file rows and `PATH_COLUMN` keep their place. Padding is unfaced; only the digits take `session-list-count`. Counts over 999 widen the column for that row only; alternative of computing the widest count per render rejected as more code for a case the user ruled out.
- **Debounce.** `REDIFF = 50 ms`. Tests already advance by `REDIFF`, not a literal.

## Risks / Trade-offs

- [`TAB` on a file now folds its group, not its root] → wanted; root folding is still one `TAB` on the heading. Spec and docs say so.
- [50 ms means a re-diff per typing pause on large files] → the diff runs off the core thread and stale answers are dropped, so the cost is background CPU only.
