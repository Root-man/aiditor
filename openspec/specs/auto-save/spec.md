# auto-save Specification

## Purpose
Keep edits to visited files safe from a crash: write them to recovery files outside the project and offer them back on the next launch.

## Requirements

### Requirement: Modified buffers are auto-saved outside the project
About 2 s after the last edit, and at least every 30 s while edits keep coming, the editor SHALL write the text of each modified buffer visiting a file that changed since its last successful auto-save to a recovery file in `recovery/` under the platform data dir. It SHALL never write anything next to the visited file or inside a project directory. Each recovery file SHALL record the original path, the buffer's name, the hash of the disk version the buffer was based on and the time, followed by the buffer's text. It SHALL be written to a temp name and renamed into place. A snapshot SHALL count as auto-saved only after its recovery file has been successfully persisted. Completion of an older snapshot SHALL leave subsequent edits eligible for auto-save, without requiring further input. Buffers visiting no file, such as `*scratch*`, `*Messages*` and the minibuffer, SHALL NOT be auto-saved, as in Emacs.

#### Scenario: Idle after an edit
- **GIVEN** a buffer visiting a file
- **WHEN** it is edited and 2 s pass with no further input
- **THEN** a recovery file holding the buffer's text exists, and the visited file's directory holds nothing new

#### Scenario: Not before the delay
- **GIVEN** a buffer visiting a file with no recovery file
- **WHEN** it is edited and less than 2 s pass
- **THEN** its first auto-save remains scheduled for the idle delay

#### Scenario: Typing without pause
- **GIVEN** a buffer visiting a file
- **WHEN** a key is typed into it every second for 30 s
- **THEN** a recovery file holding the text as of the 30 s mark exists

#### Scenario: Scratch buffer
- **GIVEN** a scratch buffer visiting no file
- **WHEN** it is edited and 2 s pass
- **THEN** its text remains available in the buffer without creating a recovery file

#### Scenario: Messages and minibuffer
- **GIVEN** `*Messages*` and an open minibuffer
- **WHEN** lines are logged and minibuffer text is typed, and 2 s pass
- **THEN** both buffers retain their text without creating recovery files

#### Scenario: Edits while an auto-save runs
- **GIVEN** an auto-save of one snapshot is still pending
- **WHEN** the buffer receives more edits and the older write succeeds
- **THEN** the recovery file initially contains that older snapshot, and the newer text is subsequently auto-saved after the normal delay without another edit

### Requirement: Recovery files are removed once not needed
A buffer's recovery file SHALL be deleted when the buffer is saved, is killed, or is no longer modified, and every recovery file SHALL be deleted when the editor quits normally. Edits made while a save runs SHALL keep the file. Cleanup SHALL also cover auto-saves already in progress, so their late completion cannot leave a discarded recovery file or restart recovery writes for a clean or killed buffer. Normal quit SHALL finish recovery cleanup without waiting for the editor to process recovery notifications.

#### Scenario: Save
- **GIVEN** an auto-saved buffer
- **WHEN** it is saved with `C-x C-s`
- **THEN** its recovery file is gone

#### Scenario: Kill
- **GIVEN** an auto-saved buffer
- **WHEN** it is killed
- **THEN** its recovery file is gone

#### Scenario: Quit
- **GIVEN** an auto-saved buffer is still modified and the user chose to exit anyway
- **WHEN** the editor quits normally
- **THEN** its recovery file is gone before the app exits

#### Scenario: Cleanup during a pending write
- **GIVEN** a buffer has an auto-save in progress
- **WHEN** the buffer is saved without further edits, reverted to a clean state, or killed before that write finishes
- **THEN** cleanup removes the write's recovery file and late success or failure leaves the buffer's cleanup complete

#### Scenario: Further edits during a regular save
- **GIVEN** a modified buffer has a recovery file and a regular file save in progress
- **WHEN** further edits arrive before that save completes
- **THEN** recovery protection remains active for the newer text and it is auto-saved after the normal delay

#### Scenario: Quit with pending recovery notifications
- **GIVEN** auto-saves are in progress and their completion notifications have not been processed
- **WHEN** the user confirms a normal quit
- **THEN** the editor finishes the queued recovery writes and cleanup and exits

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

### Requirement: Failed auto-saves report and retry
When an auto-save fails, the editor SHALL report the affected file and the failure reason without including the buffer's contents, retain the last successful recovery copy, and retry while the buffer still needs recovery protection. A retry SHALL become eligible about 2 s after failure even if no further input arrives, and SHALL use the latest buffer text. Repeated identical failures for the same file SHALL not repeat the same diagnostic until a successful auto-save resets that failure episode. A recovered buffer's earlier-session recovery file SHALL stay available until a replacement is successfully persisted or the user saves or discards the buffer. Failure to remove an older copy after successful replacement SHALL be reported as a cleanup failure and SHALL leave the successfully persisted replacement valid.

#### Scenario: Transient failure without another edit
- **GIVEN** a modified file buffer whose auto-save fails because its recovery temp path is obstructed
- **WHEN** the obstruction is removed and the retry delay elapses without another edit
- **THEN** the initial failure has been reported and a recovery file containing the buffer's text is successfully written

#### Scenario: Latest text on retry
- **GIVEN** an auto-save has failed
- **WHEN** more edits arrive before its retry
- **THEN** the retry writes the latest text and subsequent success ends that failure episode

#### Scenario: Repeated failure
- **GIVEN** the same auto-save failure continues across several retry attempts
- **WHEN** each retry fails with the same reason
- **THEN** recovery remains eligible for another attempt and the diagnostic appears once for that failure episode

#### Scenario: Failed replacement of a recovered file
- **GIVEN** an earlier-session recovery file has been accepted and the current session's first replacement write fails
- **WHEN** a later retry succeeds
- **THEN** the earlier file has remained available throughout the failure and is removed only after the replacement exists

#### Scenario: Replacement succeeds but old-copy cleanup fails
- **GIVEN** a recovered buffer whose replacement recovery file can be written but whose old copy cannot be deleted
- **WHEN** replacement finishes
- **THEN** the replacement remains a valid recovery snapshot and the user receives a cleanup diagnostic
