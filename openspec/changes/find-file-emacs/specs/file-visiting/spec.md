## MODIFIED Requirements

### Requirement: TAB and RET complete file names to full paths
`TAB` with no selection SHALL use the first matching entry. `TAB` SHALL replace the text with the directory typed so far followed by the selected entry. `RET` SHALL visit that full path; when the selected entry is a directory, `RET` SHALL instead replace the text as `TAB` does and keep the prompt open.

#### Scenario: TAB completes a name
- **GIVEN** a file prompt lists `b.txt`
- **WHEN** `b TAB` is typed
- **THEN** the text is the directory followed by `b.txt`

#### Scenario: RET enters a directory
- **GIVEN** a file prompt lists directory `sub/` holding `c.txt`
- **WHEN** `su RET` is typed
- **THEN** the prompt is still open, its text ends in `sub/` and the list is `c.txt`

## ADDED Requirements

### Requirement: find-file starts without a selected entry
When the filename query after the last slash is empty, find-file SHALL start or reset to no selected candidate and show `*/N` in the padded count field. RET with no selection SHALL visit the typed path. C-n from no selection SHALL select the first match; C-p SHALL select the last. Typing a nonempty filename query SHALL select the first match as before. Returning to an empty query or entering a directory SHALL clear the selection. A directory-only prompt SHALL retain its existing initial selection.

#### Scenario: Starting and selecting
- **GIVEN** a directory contains five entries
- **WHEN** find-file opens there and then C-n is pressed
- **THEN** the count changes from `*/5` with no highlight to `1/5` with the first candidate highlighted

#### Scenario: Typed acceptance
- **GIVEN** find-file has matching entries but no selection
- **WHEN** RET is pressed
- **THEN** the typed path is passed to normal file visiting, including its existing directory error

#### Scenario: Directory transition
- **GIVEN** a directory entry is selected
- **WHEN** RET enters it
- **THEN** its entries are listed with no selection and the count reads `*/N`

### Requirement: Filesystem entries carry metadata
Filesystem completion entries SHALL show Unix type and permission bits, decimal human-readable file size (`41 B`, `4.1 kB`, `48.2 MB`) and compact modification age (`5s ago`, `3d10h ago`). Directories SHALL show `—` as their size. Metadata SHALL be collected without blocking editor input as part of the directory listing, and SHALL remain a snapshot until another listing. A metadata failure SHALL retain the entry with `—` for unavailable fields. Symlinked directories SHALL remain navigable with a trailing slash; a symlink's mode SHALL identify it as a link. Future timestamps SHALL display `0s ago`.

#### Scenario: Listing metadata
- **GIVEN** a directory with a 41-byte file and a subdirectory
- **WHEN** its listing completes
- **THEN** both entries have mode and age annotations, the file shows `41 B`, and the directory shows `—` for size

#### Scenario: Missing target
- **GIVEN** a directory contains a dangling symlink
- **WHEN** the directory is listed
- **THEN** the link remains a candidate with link mode bits
