# Spec Delta

## MODIFIED Requirements

### Requirement: Hits stream in batches
Hits SHALL reach the core while the search runs, in batches sent at most about every 50 ms, and the search's last message SHALL say it is done. The first non-empty batch SHALL be sent as soon as it has a hit, without waiting for the interval. Every message SHALL carry the search's number given by the core. The batching deadline SHALL be checked while processing matches within a file, so a file scan that continues producing hits beyond the interval SHALL send available hits before the file finishes. Root, file and line order, cancellation behavior and the exact hit limit SHALL remain in effect. A blocking read or a region producing no matches does not introduce an additional delivery guarantee.

#### Scenario: Done
- **GIVEN** a search over a small root
- **WHEN** the search finishes
- **THEN** the core gets its hits and a final message marked done with that search's number

#### Scenario: First hit is sent at once
- **GIVEN** a search whose clock has not advanced since it started
- **WHEN** its first match is processed
- **THEN** a batch holding that hit is sent before any later match is processed

#### Scenario: Matches within a long file scan
- **GIVEN** a search is still processing matching lines in one file when the batch interval elapses
- **WHEN** the next match is processed
- **THEN** the available hits are sent before that file finishes, in line order

#### Scenario: Cancel after an intermediate batch
- **GIVEN** a long file scan has delivered an intermediate batch
- **WHEN** that search is cancelled
- **THEN** it stops between matches or files and sends no later batch or done message
