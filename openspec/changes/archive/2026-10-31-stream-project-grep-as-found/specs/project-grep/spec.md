# Spec Delta

## MODIFIED Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, with the project's `ignore` patterns applied relative to each root. Roots SHALL be eligible for concurrent traversal, without requiring one root to finish before another starts. Cross-root and cross-file discovery order SHALL be unspecified; hits from any one file SHALL retain line order. A search SHALL treat a file as binary at its first NUL byte and stop searching it there, as ripgrep does, so a small binary file gives no hits. It SHALL skip files it cannot read and a root it cannot read.

#### Scenario: Ignored and binary files
- **GIVEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **WHEN** the root is searched
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **GIVEN** roots `/r/app` and `/r/lib`, at least two workers, a slow match-free file being searched in `/r/app`, and an available worker
- **WHEN** that worker discovers a matching file in `/r/lib`
- **THEN** the hit from `/r/lib` is delivered while the earlier file is still being searched

#### Scenario: Project ignore patterns
- **GIVEN** the project's `ignore` is `["*", "!notes/"]` and each root has matching `notes/n.md` and `src/a.rs`
- **WHEN** all roots are searched
- **THEN** the search finds the matching lines in each root's `notes/n.md` only

### Requirement: Hits stream in batches
Hits SHALL reach the core in the order they are received from search workers, without waiting for earlier roots or files. The first non-empty batch SHALL be sent as soon as the first hit is received, without waiting for the batch interval. Further pending hits SHALL be flushed approximately 50 ms after the preceding batch, or immediately if received after that deadline, even when no further match or file completion occurs. Normal scheduling and downstream backpressure can delay delivery; a worker's blocking read or traversal SHALL NOT prevent the collector from flushing hits it already holds. Every message SHALL carry the search's number. A successful search SHALL end with exactly one message marked done, including its remaining accepted hits. Cancellation and the exact hit limit SHALL remain in effect.

#### Scenario: Done
- **GIVEN** a search over a small root
- **WHEN** the search finishes
- **THEN** the core gets its hits and exactly one final message marked done with that search's number

#### Scenario: First hit is sent at once
- **GIVEN** a running search has sent no hits
- **WHEN** its first hit reaches the collector
- **THEN** a batch holding that hit is sent without waiting for another hit, a file completion or the interval

#### Scenario: Matches within a long file scan
- **GIVEN** a search is still processing matching lines in one file and the collector holds pending hits
- **WHEN** the batch deadline elapses
- **THEN** the available hits are sent before that file finishes, preserving their line order

#### Scenario: Pending hits followed by a stalled worker
- **GIVEN** the first batch was delivered and a second hit is pending while all workers are blocked or producing no further events
- **WHEN** the next batch deadline elapses and the output consumer is available
- **THEN** the second hit is delivered while those workers remain stalled

#### Scenario: Cancel after an intermediate batch
- **GIVEN** a long file scan has delivered an intermediate batch
- **WHEN** that search is cancelled
- **THEN** it stops between matches or files and sends no later batch or done message

### Requirement: Hits are capped
A search SHALL accept at most the hit limit supplied by the core, counting hits in the order the collector receives them. Once that count is reached it SHALL stop accepting hits, stop further work cooperatively and end as done. When enough matches exist, exactly the limit SHALL be delivered; concurrent arrivals SHALL NOT overshoot it. Membership of a capped result set SHALL be permitted to vary with discovery order. A zero limit SHALL complete successfully with no hits and without searching files.

#### Scenario: Limit
- **GIVEN** files hold 10 matching lines and the requested limit is 3
- **WHEN** the search runs
- **THEN** the core gets the first 3 hits received by the collector and exactly one done message

#### Scenario: Simultaneous worker results reach the cap
- **GIVEN** four workers have ready matches and only one slot remains below the limit
- **WHEN** their hits reach the collector
- **THEN** the next received hit fills that slot, the final batch completes the exact cap, and later arrivals are discarded

#### Scenario: Zero limit
- **GIVEN** a valid pattern with limit zero
- **WHEN** the search starts
- **THEN** it completes with an empty done message without searching files

### Requirement: C-x p g greps the project
`C-x p g` (`project-grep`) SHALL open the minibuffer at `Grep in <project>: ` for the current workspace's project, with an empty list, and search all of the project's roots. The primary root SHALL NOT receive delivery priority over ready hits from other roots. The typed text SHALL be the search's pattern as given, and the prompt SHALL keep a history of typed texts as swiper's does.

#### Scenario: Opening
- **GIVEN** the workspace uses project `app`
- **WHEN** `C-x p g` is typed
- **THEN** the minibuffer shows `Grep in app: ` and lists nothing

#### Scenario: History
- **GIVEN** a grep for `needle` ended
- **WHEN** `C-x p g M-p` is typed
- **THEN** the text is `needle`

### Requirement: Hits are listed as they arrive
Each hit SHALL be listed as `<root name>/<path relative to the root>:<1-based line>:<line text>`, root names as `project-find-file` gives them, in batch-arrival order without matching them against the text again. The list SHALL keep the previous search's hits until the new search's first answer replaces them, and later answers SHALL append to it. Completion SHALL retain this order without sorting or replacing already listed hits. An invalid pattern SHALL show its error in the echo area and empty the list.

#### Scenario: Listed
- **GIVEN** root `app` has `src/a.rs` with `let needle = 1;` on line 3
- **WHEN** `needle` is searched
- **THEN** the list holds `app/src/a.rs:3:let needle = 1;`

#### Scenario: Old hits stay until new ones arrive
- **GIVEN** hits for `needle` are listed
- **WHEN** `s` is typed
- **THEN** those hits stay listed until the search for `needles` answers

#### Scenario: Completion preserves arrival order
- **GIVEN** the search has listed a hit in `b.rs` before a hit in `a.rs`
- **WHEN** the search completes
- **THEN** the list still shows the `b.rs` hit before the `a.rs` hit

#### Scenario: Invalid pattern
- **GIVEN** the project grep prompt is open
- **WHEN** `ab(` is searched
- **THEN** the echo area shows the regexp's error and the list is empty

### Requirement: Files are searched by a bounded number of workers
A search SHALL search files on at most the worker count it carries, at least one, each worker searching one file at a time. With any count, an uncapped search over unchanged files SHALL deliver the same hit set, preserving per-file line order and the same batching, cancellation, limit, ignore, smart case, binary, unreadable-file and Unicode-column behavior. Cross-file order and capped membership SHALL follow arrival order. The pending result queue SHALL have a fixed capacity for a configured worker count, and accumulated result batches SHALL remain bounded by the requested hit limit. Traversal SHALL start delivering work without first materializing the project's complete file inventory; traversal frontier storage is not covered by the fixed result-queue bound.

#### Scenario: Same ordered hits
- **GIVEN** unchanged files in two roots contain fewer hits than the limit
- **WHEN** the search runs with one and with four workers
- **THEN** both deliver the same complete hit set, each file's hits in line order, and exactly one done message

#### Scenario: Overlap is bounded
- **GIVEN** enough searchable files and a worker count of 3
- **WHEN** file searches overlap
- **THEN** more than one file is searched at a time and never more than 3

#### Scenario: Out-of-order completion streams in order
- **GIVEN** several workers are searching files and a later file produces a hit while an earlier file is stalled
- **WHEN** the hit reaches the collector
- **THEN** it is delivered in collector arrival order according to the batch deadline while the earlier file remains stalled

#### Scenario: Cancel with full queues
- **GIVEN** result queues have filled while several workers search matching files
- **WHEN** the search is cancelled and the output callback returns
- **THEN** blocked result producers are released, workers stop cooperatively and the search returns without further messages

#### Scenario: Limit with workers
- **GIVEN** four workers search files holding many matching lines with limit 3
- **WHEN** the collector receives three hits
- **THEN** the core gets exactly those three hits and one done message, and workers terminate cooperatively

### Requirement: Streamed hits leave the list in place
A batch of hits added to the running search SHALL leave the selected hit, its preview and the shown rows as they were. Completion SHALL also preserve that selection and order. Only the new search's first answer SHALL return the selection to the first hit.

#### Scenario: A batch arrives after the selection moved
- **GIVEN** the running search has listed hits and `C-n` selected another hit
- **WHEN** another batch arrives
- **THEN** the same hit stays selected, its preview remains current and the same rows are shown

#### Scenario: Search completes after selection moved
- **GIVEN** arrival-order hits are listed and the user selected a hit that is not first
- **WHEN** the final batch arrives
- **THEN** new hits append and the selected hit, preview and displayed ordering are preserved
