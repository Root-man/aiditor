## MODIFIED Requirements

### Requirement: Project roots are watched
Every root of every project bound to a live workspace SHALL be watched when its `watch` setting is true and its filtered counts fit `max_watched_files`; roots of a project with `watch = false` SHALL contribute no root watches. A root stops being watched once no live workspace's project has it. Changes under directories the file index leaves out SHALL NOT be reported, nor changes to existing files it leaves out that no buffer visits. On macOS each root SHALL get one recursive watch. On Linux each directory the index's walk keeps SHALL get its own non-recursive watch, ignored directories getting none; a directory created later SHALL get a watch unless ignored, and the files already in it SHALL be reported.

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

## ADDED Requirements

### Requirement: Project watches have a filtered size limit
Before adding project root watches, the editor SHALL count files and kept directories, including each root, after all index ignore filters across every project root. If either count exceeds the effective `max_watched_files`, it SHALL contribute no root watches, show `[unwatched]`, refresh its index only on demand or reload, and keep visited files watched through their parent directories. The echo area and `*Messages*` SHALL explain the project, counts, limit and setting name. An unchanged refusal SHALL NOT be reported on every message. Index results and reloads SHALL reevaluate eligibility.

#### Scenario: Threshold and explanation
- **GIVEN** a project with two kept files and one directory, and limit one
- **WHEN** its workspace opens and the count completes
- **THEN** the project is unwatched and the echo and `*Messages*` explain the counts and `max_watched_files = 1`

#### Scenario: At the limit
- **GIVEN** a project with two kept files and one directory, and limit two
- **WHEN** its workspace opens and the count completes
- **THEN** its roots are watched

#### Scenario: Ignored trees
- **GIVEN** a project whose ignored tree exceeds its limit but whose kept files and directories each fit
- **WHEN** its count completes
- **THEN** the kept roots are watched

#### Scenario: Empty directories and zero
- **GIVEN** a project with more kept directories than the limit, even with no files
- **WHEN** its count completes
- **THEN** it is unwatched; zero also refuses an empty root because the root itself is counted

#### Scenario: A limit crossing
- **GIVEN** an eligible project gains files over its limit
- **WHEN** a watcher-triggered reindex finishes
- **THEN** root watches stop, visited buffers still revert, and a manual reindex after reducing the files can resume root watching
