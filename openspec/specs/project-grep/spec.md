# project-grep Specification

## Purpose

Searches the files under a project's roots for a pattern off the core thread, streaming hits to the core, so project grep can show results while it types and drop searches it no longer wants.

## Requirements

### Requirement: What a search covers
A search SHALL look in the same files the file index lists for each root, the project's `ignore` patterns applied, roots in the order given and files in path order. It SHALL treat a file as binary at its first NUL byte and stop searching it there, as ripgrep does, so a small binary file gives no hits. It SHALL skip files it cannot read and a root it cannot read.

#### Scenario: Ignored and binary files
- **WHEN** a root has `.gitignore` with `target/`, and `target/out`, `src/a.rs` and a file `blob` with a NUL byte all hold `needle`
- **THEN** the search finds `needle` in `src/a.rs` only

#### Scenario: Order
- **WHEN** roots `/r/app` and `/r/lib` both hold matching files
- **THEN** every hit under `/r/app` arrives before any hit under `/r/lib`, each root's in path order

#### Scenario: Project ignore patterns
- **WHEN** the project's `ignore` is `["*", "!notes/"]` and `notes/n.md` and `src/a.rs` both hold `needle`
- **THEN** the search finds `needle` in `notes/n.md` only

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

### Requirement: C-x p g greps the project
`C-x p g` (`project-grep`) SHALL open the minibuffer at `Grep in <project>: ` for the current workspace's project, with an empty list, and search all of the project's roots, the primary first. The typed text SHALL be the search's pattern as given, and the prompt SHALL keep a history of typed texts as swiper's does.

#### Scenario: Opening
- **WHEN** in a workspace on project `app`, `C-x p g` is typed
- **THEN** the minibuffer shows `Grep in app: ` and lists nothing

#### Scenario: History
- **WHEN** a grep for `needle` ended, and `C-x p g M-p` is typed
- **THEN** the text is `needle`

### Requirement: Searches start after a pause in typing
A search SHALL start once the text has at least 3 chars and 200 ms have passed since it last changed, each change restarting the wait. A text shorter than 3 chars SHALL cancel any running search and empty the list after the same wait. At most 10,000 hits SHALL be listed.

#### Scenario: Debounced
- **WHEN** `nee` is typed at the prompt and 100 ms pass
- **THEN** no search has started, and once 200 ms have passed since the last key, one search for `nee` starts

#### Scenario: Too short
- **WHEN** hits for `needle` are listed and the text is cut to `ne`
- **THEN** after 200 ms the list is empty

### Requirement: Hits are listed as they arrive
Each hit SHALL be listed as `<root name>/<path relative to the root>:<1-based line>:<line text>`, root names as `project-find-file` gives them, in the order the search finds them, without matching them against the text again. The list SHALL keep the previous search's hits until the new search's first answer replaces them, and later answers SHALL add to it. An invalid pattern SHALL show its error in the echo area and empty the list.

#### Scenario: Listed
- **WHEN** root `app` has `src/a.rs` with `let needle = 1;` on line 3 and `needle` is searched
- **THEN** the list holds `app/src/a.rs:3:let needle = 1;`

#### Scenario: Old hits stay until new ones arrive
- **WHEN** hits for `needle` are listed and `s` is typed
- **THEN** the hits for `needle` stay listed until the search for `needles` answers

#### Scenario: Invalid pattern
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
`RET` SHALL visit the selected hit's file in the selected window, reading it first when no buffer visits it, and put point at the start of the hit's first match. With nothing listed, `RET` SHALL fail with `Match required`.

#### Scenario: Search, move, jump
- **WHEN** `a.rs` holds `needle` on line 1 and `b.rs` holds `  needle` on line 2, and `C-x p g needle`, a 200 ms pause, `C-n RET` is typed
- **THEN** the current buffer visits `b.rs` with point at line 2, column 2

### Requirement: Leaving the prompt cancels the search
Closing the prompt in any way while a search runs SHALL cancel that search, and answers from a cancelled or superseded search SHALL be ignored.

#### Scenario: C-g cancels
- **WHEN** a search is running and `C-g` is typed
- **THEN** the search is cancelled

#### Scenario: A late answer is dropped
- **WHEN** a search for `nee` is superseded by one for `need`, and the first one's hits arrive late
- **THEN** they are not listed
