# Design

## Context

`grep::search_with_clock` walks each root with `files::walk` (sorted, ignore rules applied) and runs one `grep_searcher::Searcher` over each file, batching hits on a ~50 ms clock and checking `cancelled` before each file, after each match and before each send. The #5 investigation found opening files dominates on macOS; traversal is ~28 ms. `ignore::WalkBuilder::build_parallel` loses the sort order, so the walk stays serial.

## Goals / Non-Goals

**Goals:** overlap file opens and searches across N workers with ordered, streaming delivery and bounded memory and handles; one code path for every N.

**Non-Goals:** a parallel walk; caching file handles or contents between searches; a cap on `grep_workers` beyond it being positive; timing thresholds in CI.

## Decisions

- **Count travels with the search.** `GrepSearch` gains `workers: NonZeroUsize`, filled from `Settings::grep_workers` when the core starts the search. The service holds no settings, and a reload naturally affects only later searches. `NonZeroUsize` in `Settings` makes serde reject 0, negatives and strings with the key in the message, reusing the existing error path.
- **Coordinator plus scoped workers.** The calling thread (the blocking-pool task) is the coordinator: it builds the matcher, walks the roots in order and owns batching, the clock, the limit and all sends. It starts N `std::thread::scope` workers that pull jobs from one shared FIFO job queue (`Mutex<mpsc::Receiver>`), each with its own `Searcher` and a clone of the matcher. Scoped threads let workers borrow the op and matcher, and the scope joins them before `search` returns.
- **Per-file bounded result channel.** Each job carries the sending half of its own `sync_channel` of hits; dropping it marks the file finished. The coordinator keeps the receivers in file order and only ever reads the front. Because workers take jobs in FIFO order, the front file is always already taken by a worker that is busy only with it, so the coordinator can block on it without deadlock while later files' workers block on their own full channels. Alternatives: one shared result channel needs unbounded reordering buffers or deadlocks; round-robin per-worker channels stall on one large file.
- **Bounds.** At most `4 * N` files are outstanding (walked but not fully delivered); each holds at most 64 hits. Before walking another file past the window the coordinator drains the front; after each dispatch it drains whatever the front already has without blocking, so hits stream while the walk is still going. Open handles: at most one per worker.
- **Cancellation and limit.** `cancelled` becomes `Fn() -> bool + Sync`, checked by workers before each file and after each match, and by the coordinator before each file, after each delivered hit and before each send. When the coordinator stops (cancel, limit or invalid state) it sets a shared stop flag and drops all receivers and the job sender, so a worker blocked on a full channel gets a send error and every worker exits. A worker also stops a file at `limit` hits since no single file needs more.

## Risks / Trade-offs

- [Thread spawn per search] → N short-lived threads per search; negligible next to the measured opens. 1 worker still spawns one thread, keeping one code path.
- [Large match-free region in the front file] → delivery of later files waits for it, as the existing spec allows; bounded by the window.
- [Cancel latency in a match-free region] → unchanged from today: workers check between files and matches only.
- [macOS speedup unverified in production] → benchmarks for 1 and 4 workers; macOS numbers are recorded by hand.
