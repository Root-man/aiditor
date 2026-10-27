## MODIFIED Requirements

### Requirement: The file prompt completes over directory entries
The `Find file: ` prompt SHALL list the entries of the directory named by the text up to its last `/`, sorted by name, each subdirectory's name followed by `/`, fuzzy-matched against the text after the last `/`. The listing SHALL be read without blocking the editor, the list SHALL fill when it arrives, and it SHALL follow the text into another directory. A directory that cannot be listed SHALL show the error in the echo area and list nothing. Only the listing the open prompt last asked for SHALL fill its list: a listing asked for by a closed prompt, or by the same prompt before its text left that directory and came back, SHALL be ignored when it arrives, even when it names the same directory.

#### Scenario: Entries of the buffer's directory
- **WHEN** a directory holds `a.txt`, `b.txt` and a directory `sub`, a buffer visits `a.txt` and `C-x C-f` is typed
- **THEN** the list is `a.txt`, `b.txt`, `sub/`

#### Scenario: Matching the last component
- **WHEN** at that prompt `b` is typed
- **THEN** the list holds `b.txt` and `sub/` and not `a.txt`

#### Scenario: Following the text
- **WHEN** at that prompt `sub/` is typed and `sub` holds `c.txt`
- **THEN** the list is `c.txt`

#### Scenario: A reopened prompt ignores the old listing
- **WHEN** `C-x C-f` is typed, `C-g` closes the prompt before its listing arrives, `C-x C-f` is typed again, and the first listing (holding `old.txt`) arrives before the second (holding `new.txt`)
- **THEN** the list is `new.txt`, and the first listing never fills it
