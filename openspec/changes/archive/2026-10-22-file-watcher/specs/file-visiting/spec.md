# Spec Delta

## MODIFIED Requirements

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
