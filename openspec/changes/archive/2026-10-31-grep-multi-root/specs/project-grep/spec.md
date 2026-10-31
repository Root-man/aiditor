# Spec Delta

## MODIFIED Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, with the project's `ignore` patterns applied relative to each root. Roots SHALL be searched sequentially in the supplied order, finishing one root before starting the next. Cross-file discovery order within a root SHALL be unspecified; hits from any one file SHALL retain line order. A search SHALL treat a file as binary at its first NUL byte and stop searching it there, as ripgrep does, so a small binary file gives no hits. It SHALL skip files it cannot read and a root it cannot read.

#### Scenario: Ignored and binary files
- **GIVEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **WHEN** the root is searched
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **GIVEN** roots `/r/app` and `/r/lib`, at least two workers, and matching files in both roots
- **WHEN** the roots are searched in that order
- **THEN** all hits from `/r/app` precede those from `/r/lib`

#### Scenario: Project ignore patterns
- **GIVEN** the project's `ignore` is `["*", "!notes/"]` and each root has matching `notes/n.md` and `src/a.rs`
- **WHEN** all roots are searched
- **THEN** the search finds the matching lines in each root's `notes/n.md` only

### Requirement: Hits stream in batches
Hits SHALL reach the core in the order they are received from search workers, without waiting for earlier files within the current root. The first non-empty batch SHALL be sent as soon as the first hit is received, without waiting for the batch interval. Further pending hits SHALL be flushed approximately 50 ms after the preceding batch, or immediately if received after that deadline, even when no further match or file completion occurs. Normal scheduling and downstream backpressure can delay delivery; a worker's blocking read or traversal SHALL NOT prevent the collector from flushing hits it already holds. Every message SHALL carry the search's number. A successful search SHALL end with exactly one message marked done, including its remaining accepted hits. Cancellation and the exact hit limit SHALL remain in effect.

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

### Requirement: C-x p g greps the project
`C-x p g` (`project-grep`) SHALL open the minibuffer at `Grep in <project>: ` for the current workspace's project, with an empty list, and search all of the project's roots. The current buffer's containing root SHALL be searched first, followed by the remaining roots in project order. If multiple roots contain its visited file, the most specific SHALL take priority. A buffer without a visited file in a project root SHALL use project order. Root names SHALL remain those assigned in project order, even when traversal order changes. The typed text SHALL be the search's pattern as given, and the prompt SHALL keep a history of typed texts as swiper's does.

#### Scenario: Opening
- **GIVEN** the workspace uses project `app`
- **WHEN** `C-x p g` is typed
- **THEN** the minibuffer shows `Grep in app: ` and lists nothing

#### Scenario: History
- **GIVEN** a grep for `needle` ended
- **WHEN** `C-x p g M-p` is typed
- **THEN** the text is `needle`

#### Scenario: Current buffer belongs to an additional root
- **GIVEN** project roots `app`, `lib`, `docs` and the current buffer visits a file in `lib`
- **WHEN** `C-x p g` searches for a pattern
- **THEN** all `lib` hits precede `app` hits, followed by `docs` hits

#### Scenario: Outside the project roots
- **GIVEN** the current buffer is scratch or visits a file outside the project's roots
- **WHEN** `C-x p g` searches for a pattern
- **THEN** hits follow project root order

#### Scenario: Nested roots
- **GIVEN** roots `/r/app` and `/r/app/lib` and the current file is `/r/app/lib/a.rs`
- **WHEN** project grep starts
- **THEN** `/r/app/lib` is searched first and each root's hits use paths relative to that searched root

## ADDED Requirements

### Requirement: Prefix selects one root
With a prefix argument, `project-grep` SHALL first prompt `Grep root: ` with completion over the project's root names, using the same unique names as the file finder. Accepting a root SHALL open `Grep in <root name>: ` and search only that root with the project's ignore patterns and the usual query history, debounce, status, preview and jump behavior. Root-picker text SHALL NOT start a grep. Cancelling either prompt SHALL return to the original buffer without starting further work.

#### Scenario: Select a root
- **GIVEN** roots `app` and `lib`
- **WHEN** `C-u C-x p g` is typed, `lib` is selected and `needle` is entered
- **THEN** the root picker completes root names and only `lib` is searched after the normal debounce

#### Scenario: Duplicate names
- **GIVEN** roots `/a/lib` and `/b/lib`
- **WHEN** `lib<2>` is selected at the root prompt
- **THEN** only `/b/lib` is searched and its hits still use `lib<2>`

#### Scenario: Cancel the root picker
- **GIVEN** the root picker is open and text has been entered
- **WHEN** `C-g` is typed
- **THEN** the original buffer is restored without a search
