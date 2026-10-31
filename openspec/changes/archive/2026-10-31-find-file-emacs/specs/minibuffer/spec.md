## MODIFIED Requirements

### Requirement: C-n and C-p move the selection
`C-n` or `<down>` SHALL select the next candidate and `C-p` or `<up>` the previous one, each repeated by the prefix argument and stopping at the first and last candidate. Any change to the text SHALL select the first match again, except find-file with an empty filename query and project-find-file with an empty query SHALL clear the selection. In either file finder, C-n from no selection SHALL select the first candidate and C-p the last. These keys SHALL keep their global bindings in a minibuffer without candidates.

#### Scenario: Moving down and back
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n C-n C-n` is typed, then `C-p`
- **THEN** `cherry` is selected, then `banana`

#### Scenario: Typing resets the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n` then `a` is typed
- **THEN** the first match is selected

### Requirement: TAB completes and RET picks the selected candidate
`TAB` SHALL replace the minibuffer's text with the selected candidate, point at its end. `RET` SHALL return the selected candidate as the string, or the text as typed when no candidate is selected. In either file finder, TAB with no selection SHALL complete the first match when one exists.

#### Scenario: TAB inserts the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n TAB` is typed
- **THEN** the text is `banana` with point at its end

#### Scenario: RET returns the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `ch RET` is typed
- **THEN** the command gets the string `cherry`

#### Scenario: RET with no match returns the text
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `zzz RET` is typed
- **THEN** the command gets the string `zzz`

### Requirement: The frame display shows the completion list
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected, and a count of the selected match's number and the number of matches, as ` 13/15 ` (`   0/0 ` with no match in ordinary completion, `   */N ` with no selection in either file finder). The frontend SHALL draw the count before the prompt and the candidates one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **GIVEN** a prompt completes over 15 candidates
- **WHEN** `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected, and its count is ` 13/15 `

## ADDED Requirements

### Requirement: Filesystem completion uses directory faces and aligned annotations
Filesystem completion SHALL draw directory names in the theme's `directory` face and ordinary filenames in the default face, with matched characters still in `completion-match`. Metadata SHALL use the `annotation` face in right-aligned mode, size and age columns ending at the frame's right inset. It SHALL never participate in fuzzy matching or match highlighting. Long names SHALL truncate before annotations rather than displacing their columns; in a narrow frame annotations SHALL remain clipped to the row.

#### Scenario: Directory and file faces
- **GIVEN** find-file lists `sub/` and `a.txt`
- **WHEN** the list is drawn
- **THEN** `sub/` uses the directory face and `a.txt` the default face, with any query highlights layered over those faces

#### Scenario: Dimmed metadata is outside matching
- **GIVEN** a file's annotation contains `41 B` and its name does not match `41`
- **WHEN** `41` is typed
- **THEN** that file is omitted from matches

#### Scenario: Long filenames
- **GIVEN** filesystem entries with long names and metadata
- **WHEN** the frame narrows
- **THEN** names are clipped before the dimmed columns and the annotations stay within the row

#### Scenario: Project file annotations
- **GIVEN** project-find-file displays indexed paths with metadata
- **WHEN** the list is drawn or narrowed
- **THEN** it uses the same dimmed right-aligned metadata columns and name clipping as find-file
