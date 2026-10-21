# file-visiting Specification

## Purpose
Opening a file named on the command line into a buffer and writing the buffer back to that file, without losing edits when the write fails.

## Requirements

### Requirement: A file named on the command line opens in a buffer
When the app is started with a file path, it SHALL read that file and make a buffer visiting it current, with point at the start and the buffer unmodified. A UTF-8 byte-order mark SHALL be left out of the buffer's text, and when the file's first line break is CRLF every CRLF SHALL become LF in the buffer. The buffer SHALL remember the BOM, the line ending and the file's content as loaded.

#### Scenario: Existing file
- **WHEN** the app starts with the path of a file containing `hello`
- **THEN** the current buffer is named after the file, holds `hello`, visits that path and is unmodified

#### Scenario: Missing file
- **WHEN** the app starts with a path where no file exists
- **THEN** the current buffer is empty, visits that path and is unmodified, and nothing is written to disk until it is saved

#### Scenario: CRLF file
- **WHEN** the file holds `a\r\nb\r\n`
- **THEN** the buffer holds `a\nb\n`

#### Scenario: File with a BOM
- **WHEN** the file starts with the UTF-8 BOM followed by `hi`
- **THEN** the buffer holds `hi`

#### Scenario: Binary or non-UTF-8 file
- **WHEN** the file has a NUL byte in its first 8 KB, or is not valid UTF-8
- **THEN** the current buffer visiting it is empty and read-only, the echo area names the file and the reason, and the app still starts

#### Scenario: Unreadable file
- **WHEN** the file exists but cannot be read, for example for lack of permission
- **THEN** the app exits with an error message naming the file and does not open a window

### Requirement: C-x C-s saves the current buffer to its file
`C-x C-s` SHALL write the current buffer's text to the file it visits, with the BOM and line ending the file was loaded with, and a final newline added to the buffer first when its non-empty text lacks one. The write SHALL be refused, with a message naming the file, when the file on disk differs from what the buffer last loaded or saved, including a file that appeared or disappeared since. The buffer SHALL become unmodified only after the write succeeds, and only if it was not edited while the write ran; the echo area SHALL then say `Wrote <path>`. A failed write SHALL show its error in the echo area.

#### Scenario: Save after editing
- **WHEN** a buffer visiting a file is edited and `C-x C-s` is pressed
- **THEN** the file on disk holds the buffer's text, the buffer is unmodified and the echo area says `Wrote <path>`

#### Scenario: File changed on disk
- **WHEN** another program changes the file after it was loaded, and the edited buffer is saved
- **THEN** the file keeps the other program's content, the buffer stays modified, and the echo area says the file changed on disk

#### Scenario: CRLF and BOM survive a save
- **WHEN** a file loaded with a BOM and CRLF line endings is edited and saved
- **THEN** the file starts with the BOM and every line ends with CRLF

#### Scenario: Missing final newline
- **WHEN** a buffer whose text is `abc` is saved
- **THEN** the buffer and the file hold `abc\n`

#### Scenario: Unmodified buffer
- **WHEN** `C-x C-s` is pressed in an unmodified buffer
- **THEN** nothing is written and the echo area says `(No changes need to be saved)`

#### Scenario: Failed write keeps the edits
- **WHEN** the write fails, for example because the file's directory does not exist
- **THEN** the echo area shows the error and the buffer stays modified with its edits, plus the final newline the save added

#### Scenario: Buffer without a file
- **WHEN** `C-x C-s` is pressed in a buffer that visits no file
- **THEN** nothing is written and the buffer stays as it was

#### Scenario: C-x followed by another key
- **WHEN** `C-x` is followed by any key other than `C-s`
- **THEN** neither key edits the buffer and nothing is saved

### Requirement: Saves keep the file's mode and links
A save SHALL replace an existing file atomically, so a crash mid-save leaves the old or the new content, never a truncated file, and SHALL give the new file the old file's permissions. Saving through a symlink SHALL replace the link's target and keep the link. A file with more than one hard link, or in a directory where no temp file can be made, SHALL be written in place so links stay shared.

#### Scenario: Permissions survive
- **WHEN** a file with mode 0754 is saved
- **THEN** it still has mode 0754

#### Scenario: Symlink
- **WHEN** a buffer visiting a symlink is saved
- **THEN** the path is still a symlink and its target holds the new text

#### Scenario: Hard link
- **WHEN** a file with a second hard link is saved
- **THEN** both names show the new text

### Requirement: Read-only buffers refuse edits
An edit to a read-only buffer SHALL fail with the message `Buffer is read-only` and leave the text unchanged.

#### Scenario: Typing in a refused binary file
- **WHEN** a key is typed in the empty buffer of a refused binary file
- **THEN** the buffer stays empty and the echo area says `Buffer is read-only`

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
When a buffer of the current workspace already visits the named path, `find-file` SHALL make that buffer current without reading the file and without making another buffer. A buffer visiting the path in another workspace SHALL NOT be reused: the file SHALL be read into a new buffer of the current workspace.

#### Scenario: Same file twice
- **WHEN** `d/a.txt` is visited, edited, and `C-x C-f` names `d/a.txt` again
- **THEN** there is one buffer visiting it, it is current and keeps its edits

#### Scenario: Same file in another workspace
- **WHEN** `d/a.txt` is visited, a new workspace is made, and `d/a.txt` is visited again
- **THEN** there are two buffers visiting it, one per workspace, and the new workspace's is current

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

### Requirement: C-x C-f previews the selected file
`find-file` SHALL preview the selected candidate's file in the window it was opened from, as a previewing prompt does; a selected directory SHALL show that window as it was.

#### Scenario: Selecting a file
- **WHEN** `C-x C-f` lists a directory holding `b.txt` and `b.txt` is selected
- **THEN** the window shows `b.txt`'s text while the prompt is open

#### Scenario: Selecting a directory
- **WHEN** a subdirectory is selected
- **THEN** the window shows what it showed before the prompt
