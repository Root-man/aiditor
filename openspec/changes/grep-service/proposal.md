# Proposal

## Why

Project grep in the minibuffer (#74) needs a search over every root of the project that runs off the core thread, shows hits while it is still running and stops at once when the user types a new query or leaves the prompt. Searching a large repo on the core thread would freeze typing, and a search nobody waits for would keep the disk and a CPU busy (#73).

## What Changes

- A grep service: given a project's roots, a regexp and a hit limit, it searches every file the file index would list, in root order and path order, with ripgrep's library crates.
- The pattern is one regexp in ripgrep's syntax, matched case-insensitively while it has no upper-case letter (ripgrep's `--smart-case`, which consult-ripgrep passes). An invalid one ends the search with its error.
- Binary files (a NUL byte) and unreadable files are skipped, as `rg` skips them.
- Hits stream to the core in batches, at most about every 50 ms, each hit carrying its root, path relative to it, 0-based line, char column of the first match and the line's text; the last batch says the search is done.
- The search stops at the core's hit limit, and stops promptly when a newer search starts or the core cancels it; a stopped search sends nothing more.

Not in this change: the minibuffer command that runs searches and shows hits (#74), the query syntax beyond one regexp, truncating long lines, editable results (#117).

## Capabilities

### New Capabilities
- `project-grep`: searching the files under a project's roots for a pattern, streaming hits, the hit limit and cancellation.

### Modified Capabilities

## Impact

- `aiditor-core`: `Effect::Grep` with `GrepOp::Search` / `GrepOp::Cancel`, `Msg::Grepped`, the `Hit` type; no command sends them yet.
- `aiditor-services`: a `grep` module with the search and the cancel handle; the file index's walker settings shared with it. New dependencies `grep-searcher`, `grep-regex`, `grep-matcher` (ripgrep's crates, chosen in `docs/editor.md`).
- `aiditor` and `aiditor-headless`: carry out `Effect::Grep`.
- `docs/editor.md`: the grep service recorded under Subsystems.
