# Spec Delta

## MODIFIED Requirements

### Requirement: Hits are listed as they arrive
Each hit SHALL be listed under its file's separator row as its 1-based line number and line text, without matching it against the input again. A file SHALL have one group, identified by its searched root and relative path. Groups SHALL follow first-seen file order, with hits in per-file arrival order; later hits SHALL join their existing group. The separator SHALL show the relative file path, prefixed with its unique finder root name when the search covers multiple roots, on a rule across the available width. File labels SHALL use `grep-file`, rules `separator`, and hit line numbers `grep-line-number`. The list SHALL keep the previous search's hits until the new search's first answer replaces them. Completion SHALL retain the grouping order. An invalid pattern SHALL show its error in the echo area and empty the list.

#### Scenario: Listed
- **GIVEN** root `app` has `src/a.rs` with `let needle = 1;` on line 3 and root `lib` has `src/a.rs` with `needle` on line 1
- **WHEN** `needle` is searched across both roots
- **THEN** headers show `app/src/a.rs` and `lib/src/a.rs`, with hits showing `3: let needle = 1;` and `1: needle`

#### Scenario: Single root
- **GIVEN** the search covers only root `app`
- **WHEN** a hit arrives for `src/a.rs`
- **THEN** its header shows `src/a.rs` without a root prefix

#### Scenario: Old hits stay until new ones arrive
- **GIVEN** hits for `needle` are listed
- **WHEN** `s` is typed
- **THEN** those hits stay listed until the search for `needles` answers

#### Scenario: Completion preserves arrival order
- **GIVEN** hits arrive for file `b.rs`, file `a.rs`, then `b.rs` again
- **WHEN** the list is displayed and the search completes
- **THEN** one `b.rs` group holds both its hits before the `a.rs` group

#### Scenario: Invalid pattern
- **GIVEN** the project grep prompt is open
- **WHEN** `ab(` is searched
- **THEN** the echo area shows the regexp's error and the list is empty

### Requirement: Streamed hits leave the list in place
A batch of hits added to the running search SHALL retain the selected hit and its preview. The viewport SHALL retain its top hit when possible, shifting only as needed to keep selection visible after regrouping. Completion SHALL also preserve selection and grouping order. Only the new search's first answer SHALL return the selection to the first hit.

#### Scenario: A batch arrives after the selection moved
- **GIVEN** a running search lists hits in files `a.rs` and `b.rs` and the user selected the hit in `b.rs`
- **WHEN** another batch adds a hit to `a.rs`
- **THEN** the same `b.rs` hit stays selected and previewed even though its displayed index changes

#### Scenario: Search completes after selection moved
- **GIVEN** grouped hits are listed and the user selected a hit that is not first
- **WHEN** the final batch arrives
- **THEN** new hits join their file groups and the same hit stays selected and previewed

### Requirement: The prompt shows the current search state
The project grep echo row SHALL show `[searching]` at its right inset, separate from the prompt and input, from the start of the current query's search until it finishes. The existing count SHALL grow as batches arrive. A successful search that finishes with zero hits SHALL show `[no matches]` instead. A successful search with hits and a failed search SHALL show the ordinary prompt, with regexp errors still reported in the echo area. Opening the prompt, changing the input, shortening it below the minimum and closing the prompt SHALL clear the previous query's status. No status SHALL be shown during the input debounce. Replies from a cancelled or superseded search SHALL leave the current prompt status unchanged. Status SHALL be display-only, preserving prompt history and typed text.

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

The input area SHALL reserve the longest status label plus two columns throughout each query prompt, including debounce and completed states, so showing, replacing or clearing a label SHALL retain the prompt/input origin, cursor position and horizontal scroll for unchanged text and count. Root picking and other prompts SHALL retain their ordinary input width.

#### Scenario: Status transitions preserve input position
- **GIVEN** a query is typed, including one long enough to scroll horizontally
- **WHEN** status changes from absent to `[searching]`, then to `[no matches]` or clears on completion
- **THEN** status is right-aligned, the prompt remains `Grep in <project>: `, and the same input text and cursor stay at the same positions


## ADDED Requirements

### Requirement: File separators are display-only
Next and previous completion motion SHALL select hits only, including across file boundaries, and the count SHALL count hits only. The viewport SHALL show at most ten physical rows including headers, keep the selected hit visible, and show the current file header when starting within its group. Every visible header SHALL have at least one visible hit. The minibuffer SHALL retain its grow-only height until closed.

#### Scenario: Moving across files
- **GIVEN** the list has two files, with one hit in each
- **WHEN** next completion then previous completion are invoked
- **THEN** selection moves to the second hit and back to the first, the count changes from `1/2` to `2/2` and back, and accepting selects the hit rather than a header

#### Scenario: Scrolling through groups
- **GIVEN** more grouped hits than fit in ten physical rows
- **WHEN** completion motion selects a hit beyond the initial viewport
- **THEN** the selected hit is visible with its file header, the viewport has at most ten rows, and every shown header has a shown hit

#### Scenario: Results shrink
- **GIVEN** the prompt has shown ten physical rows
- **WHEN** replacement results contain one file and one hit
- **THEN** the header and hit are shown and the remaining eight rows stay empty until the prompt closes
