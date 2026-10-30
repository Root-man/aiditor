## ADDED Requirements

### Requirement: Files are searched by a bounded number of workers
A search SHALL search files on at most the worker count it carries, at least one, each worker searching one file at a time. With any count it SHALL deliver the same hits in the same root, file and line order as one worker, with the same batching, cancellation, limit, ignore, smart case, binary, unreadable-file and Unicode-column behavior. The files queued for workers and the hits held back for ordered delivery SHALL stay bounded independently of the project's size.

#### Scenario: Same ordered hits
- **WHEN** a search over two roots of many matching files runs with 4 workers
- **THEN** its hits equal a one-worker search's, in the same order, and one done message ends it

#### Scenario: Overlap is bounded
- **WHEN** a search with 3 workers runs over enough files
- **THEN** more than one file is searched at a time and never more than 3

#### Scenario: Out-of-order completion streams in order
- **GIVEN** a search with several workers where a later file finishes before an earlier one
- **WHEN** the earlier file's hits arrive
- **THEN** batches are sent in file order as hits become next in order, without waiting for the whole search

#### Scenario: Cancel with full queues
- **GIVEN** a search with several workers whose held-back hits have filled their bounds
- **WHEN** the search is cancelled
- **THEN** its workers stop and it sends nothing more, not even done

#### Scenario: Limit with workers
- **WHEN** a search with limit 3 and 4 workers runs over files holding many matching lines
- **THEN** the core gets exactly the first 3 hits in order and one done message
