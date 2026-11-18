## ADDED Requirements

### Requirement: Files too large to visit are refused
A regular file larger than 256 MiB SHALL NOT be read for visiting. `find-file`, every other visit, and every other read of the file (revert, diff, the change check and a save's disk check), SHALL fail with `<path>: file too large (<n> MiB)` in the echo area, leaving the current buffer as it was; a file named on the command line SHALL be refused as an unreadable file is. File previews keep their own smaller limit.

#### Scenario: A huge log
- **WHEN** `find-file` names a 300 MiB file
- **THEN** no bytes of it are read into a buffer, no buffer is made, and the echo area shows `<path>: file too large (300 MiB)`

#### Scenario: Just under the limit
- **WHEN** `find-file` names a 200 MiB text file
- **THEN** it is visited as any other text file

## MODIFIED Requirements

### Requirement: Editor writes preserve conflict checks under concurrency
Concurrent editor writes to the same disk file SHALL serialize their disk expectation check and write as one operation, including saves from different workspaces and project-file writes. This SHALL also hold when paths alias the same file through symbolic or hard links. Each write SHALL check the content left by preceding editor writes. A write whose bytes are exactly what the file already holds SHALL succeed without writing, whatever disk content it expected. A failed or conflicting write SHALL retain the buffer's edits and its existing error behavior. Temporary replacement files SHALL belong to one write and SHALL NOT overwrite or remove another operation's temporary file. Existing permissions, link handling and missing-file recreation rules SHALL remain in effect.

#### Scenario: Two workspace buffers save the same file
- **GIVEN** two workspaces loaded the same disk content and edited it differently
- **WHEN** both saves are issued before either finishes
- **THEN** one edit is saved, the other reports a disk conflict and keeps its edits, and the file contains the complete successful edit

#### Scenario: One buffer saved twice
- **GIVEN** an edited buffer
- **WHEN** `C-x C-s` is pressed twice, or `C-x C-s` and then a quit answered `Yes`, before the first save answers
- **THEN** both saves succeed, the buffer is unmodified, no `changed on disk` is reported and the quit goes ahead

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
