# Spec Delta

## MODIFIED Requirements

### Requirement: Hits stream in batches
Hits SHALL reach the core while the search runs, in batches sent at most about every 50 ms, and the search's last message SHALL say it is done. Every message SHALL carry the search's number given by the core. The batching deadline SHALL be checked while processing matches within a file, so a file scan that continues producing hits beyond the interval SHALL send available hits before the file finishes. Root, file and line order, cancellation behavior and the exact hit limit SHALL remain in effect. A blocking read or a region producing no matches does not introduce an additional delivery guarantee.

#### Scenario: Done
- **GIVEN** a search over a small root
- **WHEN** the search finishes
- **THEN** the core gets its hits and a final message marked done with that search's number

#### Scenario: Matches within a long file scan
- **GIVEN** a search is still processing matching lines in one file when the batch interval elapses
- **WHEN** the next match is processed
- **THEN** the available hits are sent before that file finishes, in line order

#### Scenario: Cancel after an intermediate batch
- **GIVEN** a long file scan has delivered an intermediate batch
- **WHEN** that search is cancelled
- **THEN** it stops between matches or files and sends no later batch or done message

### Requirement: RET jumps to the hit
`RET` SHALL visit the selected hit's file in the selected window, reading it first when no buffer in the current workspace visits it, and put point at the start of the hit's first match. When a read is needed, the destination SHALL belong to the same pending visit and navigation context as that read; it SHALL be discarded when that visit becomes obsolete, according to file-visiting rules. With nothing listed, `RET` SHALL fail with `Match required`.

#### Scenario: Search, move, jump
- **GIVEN** `a.rs` holds `needle` on line 1 and `b.rs` holds `  needle` on line 2
- **WHEN** `C-x p g needle`, a 200 ms pause, and `C-n RET` are typed and the visit completes in its originating context
- **THEN** the current buffer visits `b.rs` with point at line 2, column 2

#### Scenario: Another workspace becomes current
- **GIVEN** accepting a grep hit started reading its file
- **WHEN** the user switches workspace before that read answers
- **THEN** the destination workspace retains its selected buffer and point when the reply arrives

#### Scenario: An old jump shares a newer visit's path
- **GIVEN** a grep jump was invalidated and an ordinary visit to the same file was later requested
- **WHEN** the ordinary visit completes
- **THEN** it uses normal file-visiting point placement rather than the obsolete grep location
