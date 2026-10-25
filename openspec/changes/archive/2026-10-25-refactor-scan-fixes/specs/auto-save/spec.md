# Spec Delta

## MODIFIED Requirements

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
