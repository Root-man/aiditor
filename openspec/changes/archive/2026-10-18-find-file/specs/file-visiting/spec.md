# Spec Delta

## ADDED Requirements

### Requirement: C-x C-f visits a file
`find-file` (`C-x C-f`) SHALL read a file name at the prompt `Find file: `, whose text starts as the current buffer's directory with a trailing `/`, or the working directory for a buffer that visits no file. A relative name SHALL be taken from that directory. It SHALL visit the file in a new current buffer as a file named on the command line is visited, read without blocking the editor.

#### Scenario: Existing file
- **WHEN** a buffer visits `d/a.txt`, `d/b.txt` holds `hi`, and `C-x C-f b.txt RET` is typed
- **THEN** the current buffer is named `b.txt`, holds `hi`, visits `d/b.txt` and is unmodified

#### Scenario: New file
- **WHEN** at the `Find file: ` prompt the name of a file that does not exist is entered
- **THEN** the current buffer is empty, visits that path and is unmodified, and no file is written until it is saved

#### Scenario: The prompt starts at the buffer's directory
- **WHEN** a buffer visits `/x/y/a.txt` and `C-x C-f` is typed
- **THEN** the minibuffer's text is `/x/y/`

### Requirement: Visiting a file already visited switches to its buffer
When a buffer already visits the named path, `find-file` SHALL make that buffer current without reading the file and without making another buffer.

#### Scenario: Same file twice
- **WHEN** `d/a.txt` is visited, edited, and `C-x C-f` names `d/a.txt` again
- **THEN** there is one buffer visiting it, it is current and keeps its edits

### Requirement: find-file refuses files it cannot visit
A binary or non-UTF-8 file SHALL NOT be visited; the echo area SHALL name the file and the reason. A file that cannot be read, or a directory, SHALL show the error in the echo area. In each case the current buffer SHALL stay as it was.

#### Scenario: Binary file
- **WHEN** `find-file` names a file with a NUL byte
- **THEN** no buffer visits it, the echo area names it and says `binary file`

#### Scenario: Directory
- **WHEN** `find-file` names a directory
- **THEN** no buffer is made and the echo area shows the error

### Requirement: The file prompt completes over directory entries
The `Find file: ` prompt SHALL list the entries of the directory named by the text up to its last `/`, sorted by name, each subdirectory's name followed by `/`, fuzzy-matched against the text after the last `/`. The listing SHALL be read without blocking the editor, the list SHALL fill when it arrives, and it SHALL follow the text into another directory. A directory that cannot be listed SHALL show the error in the echo area and list nothing.

#### Scenario: Entries of the buffer's directory
- **WHEN** a directory holds `a.txt`, `b.txt` and a directory `sub`, a buffer visits `a.txt` and `C-x C-f` is typed
- **THEN** the list is `a.txt`, `b.txt`, `sub/`

#### Scenario: Matching the last component
- **WHEN** at that prompt `b` is typed
- **THEN** the list holds `b.txt` and `sub/` and not `a.txt`

#### Scenario: Following the text
- **WHEN** at that prompt `sub/` is typed and `sub` holds `c.txt`
- **THEN** the list is `c.txt`

### Requirement: TAB and RET complete file names to full paths
`TAB` SHALL replace the text with the directory typed so far followed by the selected entry. `RET` SHALL visit that full path; when the selected entry is a directory, `RET` SHALL instead replace the text as `TAB` does and keep the prompt open.

#### Scenario: TAB completes a name
- **WHEN** at the prompt in that directory `b TAB` is typed
- **THEN** the text is the directory followed by `b.txt`

#### Scenario: RET enters a directory
- **WHEN** at the prompt `su RET` is typed
- **THEN** the prompt is still open, its text ends in `sub/` and the list is `c.txt`

### Requirement: DEL deletes a whole directory in the file prompt
In the `Find file: ` prompt, `DEL` without a prefix argument or an active mark, with point at the end of the text, SHALL delete back to just after the previous `/` when the text ends in `/` or its last component is a listed directory. Otherwise, and in other prompts, it SHALL delete one char.

#### Scenario: Trailing slash
- **WHEN** the text is `<d>/sub/` and `DEL` is typed
- **THEN** the text is `<d>/`

#### Scenario: A directory without its slash
- **WHEN** the text is `<d>/sub`, `sub` being a directory, and `DEL` is typed
- **THEN** the text is `<d>/`

#### Scenario: A file
- **WHEN** the text is `<d>/b.txt` and `DEL` is typed
- **THEN** the text is `<d>/b.tx`
