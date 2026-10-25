# auto-save Specification

## Purpose
Keep edits to visited files safe from a crash: write them to recovery files outside the project and offer them back on the next launch.

## Requirements

### Requirement: Modified buffers are auto-saved outside the project
About 2 s after the last edit, and at least every 30 s while edits keep coming, the editor SHALL write the text of each modified buffer visiting a file that changed since its last auto-save to a recovery file in `recovery/` under the platform data dir. It SHALL never write anything next to the visited file or inside a project directory. Each recovery file SHALL record the original path, the buffer's name, the hash of the disk version the buffer was based on and the time, followed by the buffer's text. It SHALL be written to a temp name and renamed into place. Buffers visiting no file, such as `*scratch*`, `*Messages*` and the minibuffer, SHALL NOT be auto-saved, as in Emacs.

#### Scenario: Idle after an edit
- **WHEN** a buffer visiting a file is edited and 2 s pass with no further input
- **THEN** a recovery file holding the buffer's text exists, and the visited file's directory holds nothing new

#### Scenario: Not before the delay
- **WHEN** a buffer is edited and less than 2 s pass
- **THEN** no recovery file exists for it

#### Scenario: Typing without pause
- **WHEN** a key is typed into a buffer every second for 30 s
- **THEN** a recovery file holding the text as of the 30 s mark exists

#### Scenario: Scratch buffer
- **WHEN** `*scratch*` is edited and 2 s pass
- **THEN** no recovery file exists for it

#### Scenario: Messages and minibuffer
- **WHEN** lines are logged to `*Messages*` and text is typed into the minibuffer, and 2 s pass
- **THEN** no recovery file exists for either

### Requirement: Recovery files are removed once not needed
A buffer's recovery file SHALL be deleted when the buffer is saved, is killed, or is no longer modified, and every recovery file SHALL be deleted when the editor quits normally. Edits made while a save runs SHALL keep the file.

#### Scenario: Save
- **WHEN** an auto-saved buffer is saved with `C-x C-s`
- **THEN** its recovery file is gone

#### Scenario: Kill
- **WHEN** an auto-saved buffer is killed
- **THEN** its recovery file is gone

#### Scenario: Quit
- **WHEN** the editor quits with an auto-saved buffer still modified, after the user chose to exit anyway
- **THEN** its recovery file is gone before the app exits

### Requirement: Recovery files are offered on launch
At launch the editor SHALL offer each recovery file left by an earlier session, oldest first, asking whether to recover it. Recovering SHALL open the recovered text as a modified buffer: for a file buffer, one visiting the original file, whose save refuses to overwrite a file changed on disk since the text was auto-saved; for a buffer with no file, one with the recorded name. Recovering SHALL succeed even when the visited file now opens read-only, such as a file that became binary. A recovered file buffer's text SHALL be auto-saved by the current session before the old recovery file is deleted, and the old file SHALL be kept if that auto-save fails; a buffer with no file is not auto-saved, so its old recovery file is deleted once recovered. Declining SHALL delete the recovery file. Quitting the prompt SHALL keep the remaining files, and the `recover-buffers` command SHALL offer them again.

#### Scenario: Recover a file buffer
- **WHEN** a recovery file for a visited file exists at launch and the user answers `Yes`
- **THEN** the current buffer visits that file, holds the recovered text and is modified, and the old recovery file is gone

#### Scenario: Recover into a file that became read-only
- **WHEN** a recovery file holds valid text for a file whose bytes are now `[0, 1, 2]`, and the user answers `Yes`
- **THEN** the editor does not panic, and the buffer holds the recovered text, is modified and stays read-only

#### Scenario: The current session's auto-save fails
- **WHEN** a recovered file buffer's auto-save cannot be written, for example because its temp path is a directory
- **THEN** the old recovery file is kept

#### Scenario: Save after recovering
- **WHEN** a recovered file buffer is saved and the file is unchanged on disk since the auto-save
- **THEN** the file holds the recovered text

#### Scenario: Disk changed since the auto-save
- **WHEN** a recovered file buffer is saved and the file changed on disk after the auto-save
- **THEN** the save is refused as a change on disk

#### Scenario: Recover a scratch buffer
- **WHEN** a recovery file for `*scratch*` exists at launch and the user answers `Yes`
- **THEN** `*scratch*` holds the recovered text and is modified

#### Scenario: Discard
- **WHEN** the user answers `No`
- **THEN** the recovery file is deleted and no buffer is opened for it

#### Scenario: Quit the prompt
- **WHEN** the user types `C-g` at the prompt
- **THEN** the recovery file stays, and `M-x recover-buffers` offers it again
