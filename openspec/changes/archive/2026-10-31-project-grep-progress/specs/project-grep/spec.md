# Spec Delta

## ADDED Requirements

### Requirement: The prompt shows the current search state
The project grep prompt SHALL include `[searching]` from the start of the current query's search until it finishes. The existing count SHALL grow as batches arrive. A successful search that finishes with zero hits SHALL show `[no matches]` instead. A successful search with hits and a failed search SHALL show the ordinary prompt, with regexp errors still reported in the echo area. Opening the prompt, changing the input, shortening it below the minimum and closing the prompt SHALL clear the previous query's status. No status SHALL be shown during the input debounce. Replies from a cancelled or superseded search SHALL leave the current prompt status unchanged. Status SHALL be display-only, preserving prompt history and typed text.

#### Scenario: Running and streaming
- **GIVEN** the grep prompt has an eligible query
- **WHEN** its debounce expires and successive batches arrive without finishing
- **THEN** the prompt shows `[searching]` before the first batch and during later batches, with the count reflecting listed hits

#### Scenario: Finished with hits
- **GIVEN** a running query with listed hits
- **WHEN** its final batch arrives
- **THEN** the ordinary prompt and final count are shown

#### Scenario: Finished empty
- **GIVEN** a running query with no hits
- **WHEN** its successful final batch arrives empty
- **THEN** the prompt shows `[no matches]` and a zero count

#### Scenario: Failed query
- **GIVEN** an eligible invalid regexp
- **WHEN** the search reports its error
- **THEN** the ordinary prompt is shown and the echo area reports the regexp error

#### Scenario: Query changes
- **GIVEN** a prompt showing a running or empty-result status
- **WHEN** its input changes
- **THEN** the ordinary prompt is shown during the debounce, and a new eligible search shows `[searching]` when it starts

#### Scenario: Superseded reply
- **GIVEN** a newer query's search is running
- **WHEN** the previous search sends a late successful or failed final reply
- **THEN** the newer search's `[searching]` status and results remain current

#### Scenario: Cancel and reopen
- **GIVEN** a running query
- **WHEN** the prompt is cancelled and reopened and the cancelled search replies late
- **THEN** the new prompt is ordinary and empty until its own query starts

#### Scenario: Short input
- **GIVEN** a running or empty-result query
- **WHEN** the input becomes shorter than three characters and its debounce expires
- **THEN** the ordinary prompt and empty list are shown

#### Scenario: History
- **GIVEN** a query completed empty
- **WHEN** the prompt is cancelled, reopened and history recalled
- **THEN** the typed query is restored with the same prompt history as before status indicators were added
