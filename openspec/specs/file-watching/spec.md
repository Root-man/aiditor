# file-watching Specification

## Purpose
Watching the disk under the open projects and the visited files, so buffers learn when their file is deleted or changed (clean ones reverting to it) and the file index keeps up with files coming and going.

## Requirements

### Requirement: Changes are reported as debounced paths
The watcher SHALL report the paths that may have changed, collected until no new change has arrived for 100 ms, and at most 1 s after the first one, as one report. It SHALL report every change, the editor's own saves included. It SHALL NOT tell creates, deletes, renames and writes apart: a reported path means only "check this path".

#### Scenario: One write, one report
- **WHEN** another program writes a watched file once
- **THEN** exactly one report naming that file arrives

#### Scenario: An atomic save
- **WHEN** another program saves a watched file by writing a temp file and renaming it over the file, and later writes the file again
- **THEN** a report naming the file arrives for the save and another for the later write

### Requirement: Project roots are watched
Every root of every project bound to a live workspace SHALL be watched when its `watch` setting is true; roots of a project with `watch = false` SHALL contribute no root watches. A root stops being watched once no live workspace's project has it. Changes under directories the file index leaves out SHALL NOT be reported, nor changes to existing files it leaves out that no buffer visits. On macOS each root SHALL get one recursive watch. On Linux each directory the index's walk keeps SHALL get its own non-recursive watch, ignored directories getting none; a directory created later SHALL get a watch unless ignored, and the files already in it SHALL be reported.

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


#### Scenario: Unwatched project with an open file
- **GIVEN** a project with `watch = false` and a clean buffer visiting one of its files
- **WHEN** another program rewrites that file
- **THEN** a parent-directory watch reports it and the buffer auto-reverts

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

### Requirement: Clean buffers revert to a changed file
When a check finds a buffer's file present with a hash other than the buffer's disk hash, an unmodified buffer SHALL be reverted from the file as a revert does: point, mark and markers stay with their text, the old text is one undo group, and the buffer ends unmodified with the file's hash. This SHALL hold for every buffer visiting the file, in every workspace, and for a clean buffer in the deleted state or visiting a file that did not exist when visited. A buffer whose disk hash equals the file's SHALL be left alone, so a buffer's own save does not revert it. A modified buffer SHALL keep its text, its modified flag and its disk hash, and become conflicted (see file-conflicts). An auto-revert SHALL NOT echo; a failed one SHALL echo its error as `revert-buffer` does.

#### Scenario: A save in another workspace
- **WHEN** buffers in two workspaces visit the same file, one of them is edited and saved, and the watcher reports the file
- **THEN** the other buffer holds the saved text and is unmodified, and the saving buffer is not reverted

#### Scenario: Another program writes the file
- **WHEN** another program rewrites the file of an unmodified buffer whose point is on a line the change leaves alone, and the watcher reports it
- **THEN** the buffer holds the new text, is unmodified, point is on the same line, `C-/` brings the old text back, and the echo area is empty

#### Scenario: A modified buffer is not reverted
- **WHEN** another program rewrites the file of a modified buffer and the watcher reports it
- **THEN** the buffer keeps its text, stays modified and is conflicted, and `C-x C-s` asks how to resolve the conflict instead of saving

#### Scenario: A deleted file comes back with other content
- **WHEN** an unmodified buffer's file is deleted, reported, then created again with other content and reported
- **THEN** the buffer holds the new content, is not in the deleted state, and an edit followed by `C-x C-s` writes the file

### Requirement: Project ignore patterns narrow the watch
The watcher SHALL apply each project's `ignore` patterns to that project's roots as the file index does: changes under directories they leave out, and to existing files they leave out that no buffer visits, SHALL NOT be reported, and on Linux directories they leave out SHALL get no watch. A root shared by projects with different patterns SHALL be watched wherever any of them keeps a directory.

#### Scenario: An ignored file
- **WHEN** a project's `ignore` is `["*.log"]` and `a.log` and `b.rs` change under its root
- **THEN** reports name `b.rs` but not `a.log`

#### Scenario: Allow-list
- **WHEN** a project's `ignore` is `["*", "!notes/"]` and files change in `src/` and `notes/` under its root
- **THEN** reports name the file in `notes/` only, and on Linux only the root and `notes/` (with its subdirectories) are watched
