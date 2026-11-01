# file-visiting Specification

## Purpose
Opening a file named on the command line into a buffer and writing the buffer back to that file, without losing edits when the write fails.

## Requirements

### Requirement: A file named on the command line opens in a buffer
When the app is started with a file path, it SHALL read that file and make a buffer visiting it current, with point at the start and the buffer unmodified. A UTF-8 byte-order mark SHALL be left out of the buffer's text, and when every line break in the file is CRLF each SHALL become LF in the buffer. The buffer SHALL remember the BOM, the line ending and the file's content as loaded.

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

#### Scenario: A device or pipe
- **WHEN** the app starts with the path of a device or named pipe
- **THEN** the app exits with an error message naming the file, without reading it, and does not open a window

### Requirement: C-x C-s saves the current buffer to its file
`C-x C-s` SHALL write the current buffer's text to the file it visits, with the BOM and line ending the file was loaded with, and a final newline added to the buffer first when its non-empty text lacks one. The write SHALL be refused, with a message naming the file, when the file on disk differs from what the buffer last loaded or saved, including a file that appeared since. A file that is gone from disk SHALL be written back, since nothing on disk is lost. A buffer in the deleted state SHALL be written even when unmodified. The buffer SHALL become unmodified, and leave the deleted state, only after the write succeeds, and only if it was not edited while the write ran; the echo area SHALL then say `Wrote <path>`. A failed write SHALL show its error in the echo area.

#### Scenario: Save after editing
- **WHEN** a buffer visiting a file is edited and `C-x C-s` is pressed
- **THEN** the file on disk holds the buffer's text, the buffer is unmodified and the echo area says `Wrote <path>`

#### Scenario: File changed on disk
- **WHEN** another program changes the file after it was loaded, and the edited buffer is saved
- **THEN** the file keeps the other program's content, the buffer stays modified, and the echo area says the file changed on disk

#### Scenario: File deleted on disk
- **WHEN** the file is deleted after it was loaded, before the watcher reports it, and the edited buffer is saved
- **THEN** the file is written back with the buffer's text

#### Scenario: A deleted buffer
- **WHEN** an unmodified buffer is in the deleted state and `C-x C-s` is pressed
- **THEN** the file is written back with the buffer's text and the mode line no longer shows `[deleted]`

#### Scenario: CRLF and BOM survive a save
- **WHEN** a file loaded with a BOM and CRLF line endings is edited and saved
- **THEN** the file starts with the BOM and every line ends with CRLF

#### Scenario: Missing final newline
- **WHEN** a buffer whose text is `abc` is saved
- **THEN** the buffer and the file hold `abc\n`

#### Scenario: Unmodified buffer
- **WHEN** `C-x C-s` is pressed in an unmodified buffer that is not in the deleted state
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
A save SHALL replace an existing file atomically, so a crash mid-save leaves the old or the new content, never a truncated file, and SHALL give the new file the old file's permissions. Saving through a symlink SHALL replace the link's target and keep the link. A file with more than one hard link, or in a directory where no temp file can be made, SHALL be written in place so links stay shared. A file the user has no write permission for SHALL NOT be replaced: the save SHALL fail with an error naming the file.

#### Scenario: Permissions survive
- **WHEN** a file with mode 0754 is saved
- **THEN** it still has mode 0754

#### Scenario: Symlink
- **WHEN** a buffer visiting a symlink is saved
- **THEN** the path is still a symlink and its target holds the new text

#### Scenario: Hard link
- **WHEN** a file with a second hard link is saved
- **THEN** both names show the new text

#### Scenario: A file without write permission
- **GIVEN** a file with mode 0444 in a writable directory
- **WHEN** its edited buffer is saved
- **THEN** the file keeps its content, the echo area shows the error and the buffer stays modified

### Requirement: Read-only buffers refuse edits
An edit to a read-only buffer SHALL fail with the message `Buffer is read-only` and leave the text unchanged.

#### Scenario: Typing in a refused binary file
- **WHEN** a key is typed in the empty buffer of a refused binary file
- **THEN** the buffer stays empty and the echo area says `Buffer is read-only`

### Requirement: C-x C-f visits a file
`find-file` (`C-x C-f`) SHALL read a file name at the prompt `Find file: `, whose text starts as the current buffer's directory with a trailing `/`: its file's directory, a terminal buffer's directory, or the working directory for any other buffer that visits no file. A relative name SHALL be taken from that directory. It SHALL visit the file in a new current buffer as a file named on the command line is visited, read without blocking the editor.

#### Scenario: Existing file
- **WHEN** a buffer visits `d/a.txt`, `d/b.txt` holds `hi`, and `C-x C-f b.txt RET` is typed
- **THEN** the current buffer is named `b.txt`, holds `hi`, visits `d/b.txt` and is unmodified

#### Scenario: New file
- **WHEN** at the `Find file: ` prompt the name of a file that does not exist is entered
- **THEN** the current buffer is empty, visits that path and is unmodified, and no file is written until it is saved

#### Scenario: The prompt starts at the buffer's directory
- **WHEN** a buffer visits `/x/y/a.txt` and `C-x C-f` is typed
- **THEN** the minibuffer's text is `/x/y/`

#### Scenario: The prompt starts at a terminal's directory
- **WHEN** a terminal's shell ran `cd /tmp` and reported it through OSC 7, and `C-x C-f` is typed in its buffer
- **THEN** the minibuffer's text is `/tmp/`

### Requirement: Visiting a file already visited switches to its buffer
When a buffer of the current workspace already visits the named path, `find-file` SHALL make that buffer current without reading the file and without making another buffer. A buffer visiting the path in another workspace SHALL NOT be reused: the file SHALL be read into a new buffer of the current workspace.

#### Scenario: Same file twice
- **WHEN** `d/a.txt` is visited, edited, and `C-x C-f` names `d/a.txt` again
- **THEN** there is one buffer visiting it, it is current and keeps its edits

#### Scenario: Same file in another workspace
- **WHEN** `d/a.txt` is visited, a new workspace is made, and `d/a.txt` is visited again
- **THEN** there are two buffers visiting it, one per workspace, and the new workspace's is current

### Requirement: find-file refuses files it cannot visit
A binary or non-UTF-8 file SHALL NOT be visited; the echo area SHALL name the file and the reason. A file that cannot be read, a directory, or anything else that is not a regular file SHALL show the error in the echo area, and a device or pipe SHALL NOT be read. In each case the current buffer SHALL stay as it was.

#### Scenario: Binary file
- **WHEN** `find-file` names a file with a NUL byte
- **THEN** no buffer visits it, the echo area names it and says `binary file`

#### Scenario: Directory
- **WHEN** `find-file` names a directory
- **THEN** no buffer is made and the echo area shows the error

#### Scenario: A device
- **WHEN** `find-file` names `/dev/zero`
- **THEN** no buffer is made, no bytes are read from it and the echo area shows the error

#### Scenario: A named pipe
- **WHEN** `find-file` names a named pipe no program writes to
- **THEN** no buffer is made, the read does not wait for a writer and the echo area shows the error

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

### Requirement: An asynchronous visit belongs to its navigation context
A file visit started from a command SHALL belong to its originating workspace and window. Its completion SHALL be applied only while that origin is live, selected and still awaiting that visit. Switching workspace or ordinary window, deleting the origin, switching its buffer, starting a newer visit, or opening a new prompt SHALL invalidate the pending visit permanently. An invalid completion SHALL leave the current buffers, windows, prompt and echo unchanged. An accepted completion SHALL use normal workspace-local buffer reuse and file validation. Editing the originating buffer alone SHALL NOT cancel the visit.

#### Scenario: A current visit finishes
- **GIVEN** workspace `a` starts visiting an unvisited text file in its selected window
- **WHEN** the read finishes while that navigation context is still current
- **THEN** `a` owns the file buffer and its selected window shows it

#### Scenario: Switching away abandons the visit
- **GIVEN** a file read is pending in workspace `a`
- **WHEN** the user switches to `b`, returns to `a`, and the old read then finishes
- **THEN** `a` retains the buffer and window state it had after returning

#### Scenario: A newer prompt owns the interaction
- **GIVEN** an unvisited file is being opened
- **WHEN** the user opens another minibuffer before that read answers
- **THEN** the new prompt and its origin remain intact when the file reply arrives

#### Scenario: A newer visit to the same path
- **GIVEN** a visit was invalidated and another visit to the same path was issued
- **WHEN** the first request answers after the second
- **THEN** the second visit's result remains displayed and the earlier result has no effect

#### Scenario: The origin was deleted
- **GIVEN** a file read is pending for a window in workspace `a`
- **WHEN** that window or workspace is deleted before the read answers
- **THEN** the surviving workspaces retain their buffers and window configurations

### Requirement: Editor writes preserve conflict checks under concurrency
Concurrent editor writes to the same disk file SHALL serialize their disk expectation check and write as one operation, including saves from different workspaces and project-file writes. This SHALL also hold when paths alias the same file through symbolic or hard links. Each write SHALL check the content left by preceding editor writes. A failed or conflicting write SHALL retain the buffer's edits and its existing error behavior. Temporary replacement files SHALL belong to one write and SHALL NOT overwrite or remove another operation's temporary file. Existing permissions, link handling and missing-file recreation rules SHALL remain in effect.

#### Scenario: Two workspace buffers save the same file
- **GIVEN** two workspaces loaded the same disk content and edited it differently
- **WHEN** both saves are issued before either finishes
- **THEN** one edit is saved, the other reports a disk conflict and keeps its edits, and the file contains the complete successful edit

#### Scenario: Two paths reach one file
- **GIVEN** two edited buffers loaded the same file through different symlink paths or hard-link names
- **WHEN** their saves overlap
- **THEN** the later disk check sees the earlier save and refuses the conflicting edit, with the links retained

#### Scenario: Temporary name already exists
- **GIVEN** a file beside the target occupies a temporary name the editor might choose
- **WHEN** the editor saves the target
- **THEN** the target is saved using a different exclusive temporary file and the pre-existing file keeps its content

#### Scenario: Editing continues during a save
- **GIVEN** a buffer's save is queued behind another editor write
- **WHEN** the buffer is edited again before its save succeeds
- **THEN** the saved version reaches disk and the later buffer edits remain modified

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
Filesystem completion entries SHALL show Unix type and permission bits, decimal human-readable file size (`41 B`, `4.1 kB`, `48.2 MB`) and compact modification age (`5s ago`, `3d10h ago`). Directories SHALL show `—` as their size. For directory prompts, metadata SHALL be collected without blocking editor input as part of the directory listing, and SHALL remain a snapshot until another listing. A metadata failure SHALL retain the entry with `—` for unavailable fields. Symlinked directories SHALL remain navigable with a trailing slash; a symlink's mode SHALL identify it as a link. Future timestamps SHALL display `0s ago`.

#### Scenario: Listing metadata
- **GIVEN** a directory with a 41-byte file and a subdirectory
- **WHEN** its listing completes
- **THEN** both entries have mode and age annotations, the file shows `41 B`, and the directory shows `—` for size

#### Scenario: Missing target
- **GIVEN** a directory contains a dangling symlink
- **WHEN** the directory is listed
- **THEN** the link remains a candidate with link mode bits

### Requirement: Files with mixed line endings round-trip
A file whose line breaks are not all CRLF SHALL be treated as LF: its `\r` chars SHALL stay in the buffer's text and a save SHALL write them back unchanged.

#### Scenario: CRLF first, then LF
- **WHEN** a file holding `a\r\nb\n` is visited
- **THEN** the buffer holds `a\r\nb\n` and the mode line does not show `CRLF`

#### Scenario: A save changes only the edit
- **WHEN** a file holding `a\r\nb\n` is visited, `x` is typed at the end of the buffer and the buffer is saved
- **THEN** the file holds `a\r\nb\nx\n`

### Requirement: A reported save has reached the disk
Every save, whether it replaces the file, writes it in place or creates it, SHALL flush the file's content to stable storage before the buffer becomes unmodified and `Wrote <path>` is shown, so its recovery file is not deleted while the text exists only in memory.

#### Scenario: A new file
- **WHEN** a buffer visiting a file that does not exist yet is saved
- **THEN** the content is flushed before `Wrote <path>` is shown

#### Scenario: A hard-linked file
- **WHEN** a file with a second hard link is saved
- **THEN** the content is flushed before `Wrote <path>` is shown, and both names show it
