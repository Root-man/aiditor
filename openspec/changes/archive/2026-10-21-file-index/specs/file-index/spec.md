# Spec Delta

## Purpose

Keeps, for each project, the list of files under its roots in memory, so finders can offer them without walking the disk while the user types.

## ADDED Requirements

### Requirement: The index holds each root's files
A project's file index SHALL hold, for each of its roots, the primary first and then the additional ones in order, the files under that root as paths relative to it, sorted. It SHALL live in memory only: nothing is written to disk and it starts empty on every launch.

#### Scenario: Two roots
- **WHEN** a project with primary `/r/app` holding `src/main.rs` and a root `/r/lib` holding `lib.rs` is indexed
- **THEN** its index holds `src/main.rs` under `/r/app` and `lib.rs` under `/r/lib`

### Requirement: What the walk includes
The walk SHALL leave out files matched by `.gitignore` files, whether or not the root is in a git repo, by `.ignore` files and by git's global excludes. It SHALL include hidden files and directories, but never a `.git` directory or file. It SHALL list files and symlinks to files, and SHALL NOT follow symlinks to directories.

#### Scenario: Ignored and hidden files
- **WHEN** a root holds `.gitignore` with `target/`, `target/out`, `.github/ci.yml`, `.git/config` and `src/a.rs`
- **THEN** its index holds `.github/ci.yml`, `.gitignore` and `src/a.rs` only

#### Scenario: Not a git repo
- **WHEN** a root that is not in a git repo has a `.gitignore` ignoring `build/`
- **THEN** files under `build/` are not indexed

#### Scenario: Symlinked directory
- **WHEN** a root holds a symlink to a directory
- **THEN** neither the symlink nor the files under its target are indexed

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
