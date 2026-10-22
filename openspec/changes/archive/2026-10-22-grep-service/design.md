# Design

## Context

Effects reach services through `Effect::File(FileOp)`, each run once on Tokio's blocking pool and answered with one `Msg` (`editor.rs`); `aiditor-headless` runs them inline and recurses on the reply. `files::index` already walks a root with the `ignore` settings the search must share. The core can't depend on `aiditor-services`, and no core command sends a grep yet (#74 does).

## Goals / Non-Goals

**Goals:** a search that streams, stops at a limit, and stops promptly when superseded or cancelled.

**Non-Goals:** the consuming command and its debounce (#74); dropping stale messages in the core (#74, by search number); long-line truncation; parallel walking.

## Decisions

- **Its own effect, `Effect::Grep(GrepOp)`, not a `FileOp`.** `FileOp`s are one request, one reply; a search sends many messages and needs state shared across requests (the cancel), so the app holds a `grep::Grep` handle next to the recovery service.
- **Cancellation by the latest search number.** `Grep` holds an `Arc<AtomicU64>`; `Search` stores its number there on the UI thread before spawning, `Cancel` stores 0 (the core numbers from 1). The search checks `latest == search` before each file and after each match (the sink returns `false` to stop) and before each send. Storing on the UI thread makes "newest wins" independent of thread start order. Alternative, a `CancellationToken` per search: more types, same effect.
- **The search is a plain function** taking a cancel check and a send callback, so services tests drive it synchronously with a counting cancel and a collecting sink; the app wraps it in `spawn_blocking` with `blocking_send`, the bounded channel giving backpressure.
- **Batching by time:** a batch goes out when 50 ms have passed since the last send, checked after each file, plus the final one. The hit limit bounds a batch's size.
- **Walk shared with the index:** `files::walk(root)` yields the files both use, the index's walker settings and file filter in one place. `sort_by_file_name` sorts each directory, which is path order without collecting first, so the index no longer sorts.
- **Matching:** `grep-regex`'s `RegexMatcherBuilder` with `case_smart(true)`, `grep-searcher` with line numbers and `BinaryDetection::quit(b'\0')` (rg's default for walked files). The column is the char count of the line's bytes before the first match, found with `Matcher::find`; text is lossy UTF-8 with the line ending trimmed.
- **Headless:** runs a search inline, collecting its messages, then sends them to the core in order; no command emits one yet, so this only keeps the effect match whole.
- **Harness:** `Effect::Grep` is ignored until #74 gives the core a command that sends one.

## Risks / Trade-offs

- [A minified file's one huge line becomes a huge hit] → bounded by the limit; truncating is #74's call when it draws hits.
- [Sequential walk is slower than rg on huge repos] → `shortcut:` comment to switch to `build_parallel`, which would also need ordering.
- [A search blocked in `blocking_send` can't notice a cancel until the core drains] → the core drains continuously; the next check runs right after the send returns.
