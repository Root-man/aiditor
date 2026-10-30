# Proposal

## Why

Project grep on macOS spends most of a 10k-file search opening files (about 844 ms of 960 ms in the #5 investigation), one at a time. A bounded set of workers opening and searching files concurrently measured 933 ms down to 380 ms with 8 workers. Users need a way to turn that on without changing grep's results or its streaming and cancellation guarantees.

## What Changes

- New `grep_workers` setting in `settings.toml`: a positive integer, bundled as `1`. Zero, negatives and non-integers are rejected through the existing settings error path.
- The core passes the worker count it has when a search starts with the search, so a reload applies to the next search and a running search keeps its count.
- The grep service searches up to that many files at once, each worker keeping its own searcher, and delivers hits in the same root, file and line order as one worker, through the existing ~50 ms batches, cancellation checks and exact hit limit. Queued files, buffered hits and open files stay bounded.
- Service benchmarks measure one and several workers; Linux results are recorded, macOS results are measured by hand.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: searches may run file searches on several workers, bounded by the count the search carries, with unchanged ordered results.
- `settings`: adds the `grep_workers` key and its validation.

## Impact

- `crates/aiditor-core`: `settings.toml`, `Settings`, `GrepSearch` gains `workers`, `grep.rs` fills it.
- `crates/aiditor-services/src/grep.rs`: ordered parallel search; `search`'s cancel check must be callable from worker threads.
- `crates/aiditor-services/benches/services.rs`, `docs/performance.md`, `docs/editor.md`, `docs/refactoring-opportunities.md`.
- No new dependencies.
