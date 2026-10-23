# Spec Delta

## ADDED Requirements

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
