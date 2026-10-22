# file-index Specification

## Purpose
Keeps, for each project, the list of files under its roots in memory, so finders can offer them without walking the disk while the user types.

## Requirements

### Requirement: The index holds each root's files
A project's file index SHALL hold, for each of its roots, the primary first and then the additional ones in order, the files under that root as paths relative to it, sorted. It SHALL live in memory only: nothing is written to disk and it starts empty on every launch.

#### Scenario: Two roots
- **WHEN** a project with primary `/r/app` holding `src/main.rs` and a root `/r/lib` holding `lib.rs` is indexed
- **THEN** its index holds `src/main.rs` under `/r/app` and `lib.rs` under `/r/lib`

### Requirement: What the walk includes
The walk SHALL leave out files matched by `.gitignore` files, whether or not the root is in a git repo, by `.ignore` files and by git's global excludes. It SHALL also leave out what the project's `ignore` patterns match, each pattern relative to the root being walked; of the patterns matching a path or a directory above it within the root, the last SHALL decide, a negated one keeping the path. The patterns SHALL NOT bring back a file the ignore files leave out. It SHALL include hidden files and directories, but never a `.git` directory or file. It SHALL list files and symlinks to files, and SHALL NOT follow symlinks to directories.

#### Scenario: Ignored and hidden files
- **WHEN** a root holds `.gitignore` with `target/`, `target/out`, `.github/ci.yml`, `.git/config` and `src/a.rs`
- **THEN** its index holds `.github/ci.yml`, `.gitignore` and `src/a.rs` only

#### Scenario: Not a git repo
- **WHEN** a root that is not in a git repo has a `.gitignore` ignoring `build/`
- **THEN** files under `build/` are not indexed

#### Scenario: Symlinked directory
- **WHEN** a root holds a symlink to a directory
- **THEN** neither the symlink nor the files under its target are indexed

#### Scenario: Project ignore patterns
- **WHEN** a project's `ignore` is `["*.log", "docs/"]` and its root holds `a.rs`, `b.log`, `docs/x.md` and `src/c.log`
- **THEN** its index holds `a.rs` only

#### Scenario: Allow-list
- **WHEN** a project's `ignore` is `["*", "!notes/", "notes/*.tmp"]` and its root holds `a.txt`, `src/b.rs`, `notes/n.md`, `notes/deep/d.md` and `notes/x.tmp`
- **THEN** its index holds `notes/deep/d.md` and `notes/n.md` only

#### Scenario: Patterns don't override the ignore files
- **WHEN** a root's `.gitignore` lists `target/` and the project's `ignore` is `["!target/"]`
- **THEN** files under `target/` are not indexed

### Requirement: Indexing does not block typing
The walk SHALL run off the core thread, and the core SHALL only store its answer, so keys are handled while a large project is indexed.

#### Scenario: Typing during a walk
- **WHEN** a project is being indexed and the user types
- **THEN** the typed text appears before the walk is done

### Requirement: Rebuild on demand
`project-index` SHALL prompt `Project: `, completing over the loaded projects' names, echo `Indexing <project>...` and walk every root of that project. When the walk is done it SHALL replace the project's index and echo `Indexed <n> files in <project>`, `file` when `<n>` is 1, `<n>` counting every root's files. A root that could not be walked SHALL get an empty list and be named with its error after the count, `; `-separated.

#### Scenario: Index a project
- **WHEN** `project-index` is answered `app`, a project whose roots hold three files in all
- **THEN** the echo area says `Indexed 3 files in app`

#### Scenario: Missing root
- **WHEN** a root of the project no longer exists
- **THEN** the other roots are indexed and the echo area names the missing root and why it couldn't be read

### Requirement: The newest rebuild wins
When a project is indexed again before an earlier walk is done, the earlier walk's answer SHALL be dropped, whenever it arrives. An answer for a project no longer loaded SHALL be dropped.

#### Scenario: Out of order
- **WHEN** a project is indexed twice and the first walk's answer arrives after the second's
- **THEN** the index holds the second walk's files and nothing is echoed for the first

### Requirement: Creates and deletes refresh the index
When the watcher reports a path under a root of an indexed project that exists but is not in the index, or that is gone while the index holds it or files under it, the project SHALL be walked again without anything echoed, and its index replaced by the walk's answer as with `project-index`. A reported path that the index already holds and that still exists (a write) SHALL NOT start a walk, and a file the index lacks SHALL start a walk only the first time it is reported, so writes to a file the walk ignores start none. When the watcher reports lost events, every indexed project SHALL be walked again the same way.

#### Scenario: A created file
- **WHEN** a file is created under the root of an indexed project and the watcher reports it
- **THEN** the next `C-x p f` lists it, and the echo area is unchanged

#### Scenario: A deleted file
- **WHEN** an indexed file is deleted and the watcher reports it
- **THEN** the next `C-x p f` no longer lists it

#### Scenario: A write
- **WHEN** an indexed file is written and the watcher reports it
- **THEN** no walk starts

#### Scenario: An ignored file
- **WHEN** a visited file the walk ignores is reported twice
- **THEN** only the first report starts a walk

#### Scenario: Lost events
- **WHEN** the watcher reports that events were lost
- **THEN** every indexed project is walked again without anything echoed

### Requirement: A project reload re-walks the indexes
When `C-x C-r` reloads the project files, every project that has an index and is still loaded SHALL be walked again without echoing, so the index follows roots and ignore patterns edited by hand.

#### Scenario: Patterns edited by hand
- **WHEN** an indexed project's file gains `ignore = ["*.log"]` and `C-x C-r` is pressed
- **THEN** the project is walked again with `*.log` as its patterns, nothing is echoed, and its `.log` files leave the index
