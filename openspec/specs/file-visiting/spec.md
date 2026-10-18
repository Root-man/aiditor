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
