# project-grep Specification

## Purpose

Searches the files under a project's roots for a pattern off the core thread, streaming hits to the core, so project grep can show results while it types and drop searches it no longer wants.

## Requirements

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

### Requirement: A newer search cancels the older
Starting a search SHALL cancel the one running, and the core SHALL be able to cancel the running search without starting another. A cancelled search SHALL stop promptly, between files or between matches within a file, and SHALL send nothing more, not even done.

#### Scenario: Cancel a large search
- **WHEN** a search over a root of many matching files is cancelled after its first batch
- **THEN** it stops within a file of the cancel and no done message arrives for it

#### Scenario: Superseded
- **WHEN** a second search starts while the first is running
- **THEN** the first stops and the second runs to done

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

### Requirement: Searches start after a pause in typing
A search SHALL start once the text has at least 3 chars and 200 ms have passed since it last changed, each change restarting the wait. A text shorter than 3 chars SHALL cancel any running search and empty the list after the same wait. At most 10,000 hits SHALL be listed.

#### Scenario: Debounced
- **WHEN** `nee` is typed at the prompt and 100 ms pass
- **THEN** no search has started, and once 200 ms have passed since the last key, one search for `nee` starts

#### Scenario: Too short
- **WHEN** hits for `needle` are listed and the text is cut to `ne`
- **THEN** after 200 ms the list is empty

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

### Requirement: The selected hit is previewed
While the prompt is open, the window it was opened from SHALL show the selected hit's file with point at the start of the hit's first match, scrolling to center that line only when it is out of view. Leaving the prompt without a jump SHALL show that window as it was.

#### Scenario: Moving the selection moves the preview
- **WHEN** hits are listed in `a.rs` and `b.rs` and `C-n` selects the hit in `b.rs`
- **THEN** that window shows `b.rs` with point at the match, and the minibuffer stays selected

#### Scenario: Quitting goes back
- **WHEN** a hit in another file is previewed and `C-g` is typed
- **THEN** the window shows its buffer with point and scroll as before `C-x p g`

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

### Requirement: Leaving the prompt cancels the search
Closing the prompt in any way while a search runs SHALL cancel that search, and answers from a cancelled or superseded search SHALL be ignored.

#### Scenario: C-g cancels
- **WHEN** a search is running and `C-g` is typed
- **THEN** the search is cancelled

#### Scenario: A late answer is dropped
- **WHEN** a search for `nee` is superseded by one for `need`, and the first one's hits arrive late
- **THEN** they are not listed

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

### Requirement: Grep preview match faces
While a selected hit is actually previewed, its window SHALL draw every visible nonempty match with `match`, and the first match on the selected hit's line with `match-current`. Matching SHALL use the displayed results' regexp and ripgrep smart case, including while replacement results are pending. Other windows showing the same buffer SHALL keep their ordinary faces. Closing or aborting the prompt SHALL restore ordinary faces. Highlights SHALL be display overlays, not text properties.

#### Scenario: All visible matches and the selected match
- **GIVEN** a previewed file has multiple matches on the selected line and on another visible line
- **WHEN** a grep hit is selected
- **THEN** the selected line's first match uses `match-current` and every other visible match uses `match`

#### Scenario: Smart case and Unicode
- **GIVEN** a file holds `été Foo foo` and a grep for `foo` or `Foo`
- **WHEN** a hit is previewed
- **THEN** the lowercase query highlights both spellings, the uppercase query only `Foo`, and accented text keeps the match boundaries correct

#### Scenario: Replacement and delayed preview
- **GIVEN** old hits remain listed while another query searches or a selected file is still loading
- **WHEN** the frame is built
- **THEN** a shown old hit retains its displayed query's faces, and a file awaiting preview receives no premature overlay

#### Scenario: Split windows and prompt cleanup
- **GIVEN** two windows show the same file and grep previews in one
- **WHEN** the prompt is accepted or aborted
- **THEN** both windows use their ordinary faces, and during preview only the originating window had match faces

#### Scenario: Scrolled and wrapped text
- **GIVEN** a preview has scrolled vertically or horizontally, or wraps a matching line
- **WHEN** its visible text is built
- **THEN** visible portions of matches retain their corresponding match faces and regexp anchors refer to buffer lines

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
