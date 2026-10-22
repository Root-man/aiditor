# Spec Delta

## Purpose

Watching the disk under the open projects and the visited files, so buffers learn when their file is deleted (and later changed) and the file index keeps up with files coming and going.

## ADDED Requirements

### Requirement: Changes are reported as debounced paths
The watcher SHALL report the paths that may have changed, collected until no new change has arrived for 100 ms, and at most 1 s after the first one, as one report. It SHALL report every change, the editor's own saves included. It SHALL NOT tell creates, deletes, renames and writes apart: a reported path means only "check this path".

#### Scenario: One write, one report
- **WHEN** another program writes a watched file once
- **THEN** exactly one report naming that file arrives

#### Scenario: An atomic save
- **WHEN** another program saves a watched file by writing a temp file and renaming it over the file, and later writes the file again
- **THEN** a report naming the file arrives for the save and another for the later write

### Requirement: Project roots are watched
Every root of every project bound to a live workspace SHALL be watched, except the launch `$HOME` project's. A root stops being watched once no live workspace's project has it. Changes under directories the file index leaves out SHALL NOT be reported, nor changes to existing files it leaves out that no buffer visits. On macOS each root SHALL get one recursive watch. On Linux each directory the index's walk keeps SHALL get its own non-recursive watch, ignored directories getting none; a directory created later SHALL get a watch unless ignored, and the files already in it SHALL be reported.

#### Scenario: A file created in a project
- **WHEN** a file is created under a root of the current workspace's project
- **THEN** a report names it

#### Scenario: An ignored directory
- **WHEN** a root's `.gitignore` lists `node_modules/` and a file under `node_modules/` changes
- **THEN** no report names it, and on Linux `node_modules/` has no watch

#### Scenario: An ignored file
- **WHEN** a root's `.gitignore` lists `*.log`, `a.log` and `b.log` change, and a buffer visits `b.log`
- **THEN** reports name `b.log` but not `a.log`

#### Scenario: A new directory
- **WHEN** a directory holding a file is moved into a watched root, and a file in it is written later
- **THEN** reports name the moved-in file and, later, the written one

### Requirement: Visited files are watched through their directories
A file that a buffer visits, in any workspace, and that no root watch covers (outside the watched roots, under an ignored directory, or in an unwatched project) SHALL get a non-recursive watch on its parent directory. The watch SHALL be shared by every buffer visiting a file there and SHALL be dropped once no buffer visits a file in that directory. Files are always watched through their directory, never on their own.

#### Scenario: A file outside the roots
- **WHEN** a buffer visits a file outside every watched root, another program deletes it, then the buffer is killed
- **THEN** the buffer is marked deleted while it lives, and after the kill that directory is no longer watched

### Requirement: Reported paths are checked per buffer
Each file buffer, in any workspace, visiting a reported path or a file under a reported directory SHALL have its file checked on disk. When events were lost, every file buffer SHALL be checked. A file found missing SHALL put the buffer in the deleted state: it keeps its text, undo history and modified flag, and its disk hash becomes none. A file found present SHALL take the buffer out of the deleted state.

#### Scenario: A deleted file
- **WHEN** another program deletes the file an unmodified buffer visits
- **THEN** the buffer keeps its text, is still unmodified, and its mode line shows `[deleted]`

#### Scenario: A deleted directory
- **WHEN** the directory holding a visited file is deleted or renamed
- **THEN** the buffer is in the deleted state

#### Scenario: A renamed file
- **WHEN** a visited file is renamed
- **THEN** the buffer visiting the old path is in the deleted state, and no buffer follows the rename

#### Scenario: The file comes back
- **WHEN** a deleted buffer's file is created again
- **THEN** the mode line no longer shows `[deleted]`

#### Scenario: Lost events
- **WHEN** the watcher reports that events were lost
- **THEN** every buffer visiting a file is checked

#### Scenario: A clean deleted buffer doesn't stop quitting
- **WHEN** an unmodified buffer's file is deleted and `C-x C-c` is pressed
- **THEN** the editor quits without asking
