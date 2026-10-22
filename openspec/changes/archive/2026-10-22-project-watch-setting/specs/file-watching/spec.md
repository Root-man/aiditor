# Spec Delta

## MODIFIED Requirements

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
