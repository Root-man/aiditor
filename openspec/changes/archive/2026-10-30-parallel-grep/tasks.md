# Tasks

## 1. Setting

- [x] 1.1 Add `grep_workers = 1` with a comment to bundled `settings.toml` and `grep_workers: NonZeroUsize` to `Settings`; extend the settings tests for the default, a valid override and rejection of 0, -1 and "many"
- [x] 1.2 Add `workers` to `GrepSearch`, fill it from settings in `core grep.rs`, and test that a reloaded count reaches the next search's effect

## 2. Ordered parallel search

- [x] 2.1 Rewrite `grep::search_with_clock` as a coordinator with scoped workers, per-file bounded channels and a bounded window, keeping every existing grep test passing (cancel checks made `Sync`)
- [x] 2.2 Add tests: N workers give the same ordered hits as one over two roots; overlap is >1 and never above N; limit with workers gives exactly the first hits and one done; cancel with full channels stops and sends nothing more; a later file finishing first still streams in order

## 3. Benchmarks and docs

- [x] 3.1 Benchmark complete and first-batch grep with 1 and 4 workers; record Linux results in `docs/performance.md`
- [x] 3.2 Document `grep_workers` and the search behavior in `docs/editor.md` and update the status in `docs/refactoring-opportunities.md`

## Workflow follow-up

- Measure macOS complete and first-batch grep with 1 and 4 workers by hand and record them.
- Archive the change after review.
