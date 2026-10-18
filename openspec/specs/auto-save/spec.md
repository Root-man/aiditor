# auto-save Specification

## Purpose
TBD - created by archiving change auto-save-recovery. Update Purpose after archive.

## Requirements

### Requirement: Modified buffers are auto-saved outside the project
About 2 s after the last edit, and at least every 30 s while edits keep coming, the editor SHALL write the text of each modified file or scratch buffer that changed since its last auto-save to a recovery file in `recovery/` under the platform data dir. It SHALL never write anything next to the visited file or inside a project directory. Each recovery file SHALL record the original path, the buffer's name, the hash of the disk version the buffer was based on and the time, followed by the buffer's text. It SHALL be written to a temp name and renamed into place. Read-only buffers such as `*Messages*` and hidden buffers such as the minibuffer SHALL NOT be auto-saved.

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
- **THEN** a recovery file holding its text exists, with no path

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
