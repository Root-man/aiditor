# Spec Delta

## Purpose

Searches the files under a project's roots for a pattern off the core thread, streaming hits to the core, so project grep can show results while it types and drop searches it no longer wants.

## ADDED Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, roots in the order given and files in path order. It SHALL treat a file as binary at its first NUL byte and stop searching it there, as ripgrep does, so a small binary file gives no hits. It SHALL skip files it cannot read and a root it cannot read.

#### Scenario: Ignored and binary files
- **WHEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **WHEN** roots `/r/app` and `/r/lib` both hold matching files
- **THEN** every hit under `/r/app` arrives before any hit under `/r/lib`, each root's in path order

### Requirement: The pattern
The pattern SHALL be one regular expression in ripgrep's syntax, matched case-insensitively while it has no upper-case letter. An invalid pattern SHALL end the search with its error and no hits.

#### Scenario: Smart case
- **WHEN** the files hold `Foo` and `foo` and the pattern is `foo`
- **THEN** both lines are hits, and with pattern `Foo` only the first is

#### Scenario: Invalid pattern
- **WHEN** the pattern is `(`
- **THEN** the search ends at once with the regexp's error

### Requirement: What a hit holds
Each hit SHALL hold its root, its file's path relative to that root, its 0-based line number, the 0-based char column where the first match on the line starts, and the line's text without its line ending. A line SHALL be one hit however many matches it holds.

#### Scenario: A hit
- **WHEN** `src/a.rs`, line 2, is `let été = needle;` and the pattern is `needle`
- **THEN** the hit is `src/a.rs`, line 1, column 10, text `let été = needle;`

### Requirement: Hits stream in batches
Hits SHALL reach the core while the search runs, in batches sent at most about every 50 ms, and the search's last message SHALL say it is done. Every message SHALL carry the search's number given by the core.

#### Scenario: Done
- **WHEN** a search over a small root finishes
- **THEN** the core gets its hits and a final message marked done with that search's number

### Requirement: Hits are capped
A search SHALL stop once it has found the hit limit the core gave it, and SHALL then end as done.

#### Scenario: Limit
- **WHEN** a search with limit 3 runs over files holding 10 matching lines
- **THEN** the core gets exactly 3 hits and a done message

### Requirement: A newer search cancels the older
Starting a search SHALL cancel the one running, and the core SHALL be able to cancel the running search without starting another. A cancelled search SHALL stop promptly, between files or between matches within a file, and SHALL send nothing more, not even done.

#### Scenario: Cancel a large search
- **WHEN** a search over a root of many matching files is cancelled after its first batch
- **THEN** it stops within a file of the cancel and no done message arrives for it

#### Scenario: Superseded
- **WHEN** a second search starts while the first is running
- **THEN** the first stops and the second runs to done
