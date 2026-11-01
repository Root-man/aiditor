# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

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
