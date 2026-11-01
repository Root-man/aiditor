# vcs-branch Specification

## Purpose
Tells a file buffer which branch its repository is on and keeps that current when the repository changes outside the editor.

## Requirements

### Requirement: A visited file knows its branch
The editor SHALL find the nearest `.git` at or above a visited file's directory, off the UI thread, and read its branch from `HEAD`. A `.git` file SHALL be followed through its `gitdir:` line. A file with no repository SHALL have no branch.

#### Scenario: A file in a repository
- **WHEN** a file is visited in a repository whose `HEAD` is `ref: refs/heads/main`
- **THEN** its branch is `main`

#### Scenario: A branch with a slash
- **WHEN** `HEAD` is `ref: refs/heads/fix/watcher`
- **THEN** the branch is `fix/watcher`

#### Scenario: A detached head
- **WHEN** `HEAD` is a commit hash
- **THEN** the branch is the hash's first 7 characters

#### Scenario: A linked worktree
- **WHEN** a file is visited under a directory whose `.git` is a file naming a `gitdir:` with its own `HEAD`
- **THEN** the branch comes from that `HEAD`

#### Scenario: No repository
- **WHEN** a file is visited with no `.git` at or above its directory
- **THEN** it has no branch

### Requirement: A visited file knows whether git reports it changed
A file in a repository SHALL count as changed when git reports it with staged or unstaged changes or as untracked, and not when it is committed or ignored. Finding this SHALL NOT write to the repository or run a command from its config, and with no usable git the file SHALL count as unchanged. A content filter defined by the repository's own config SHALL make the file count as unchanged, since asking git would run it; filters from the user's global or system config SHALL stay in effect.

#### Scenario: An unstaged edit
- **WHEN** a committed file is changed on disk
- **THEN** it counts as changed

#### Scenario: A staged edit
- **WHEN** that change is staged
- **THEN** it counts as changed

#### Scenario: Committed
- **WHEN** the file matches its last commit
- **THEN** it does not count as changed

#### Scenario: Another file changes
- **WHEN** a file whose name its own name would match as a pattern is changed
- **THEN** it does not count as changed

#### Scenario: The index is left alone
- **WHEN** a file's status is read
- **THEN** `.git/index` is unchanged

#### Scenario: A repository's own filter
- **GIVEN** a repository whose `.git/config` defines `filter.x.clean` and whose `.gitattributes` applies `filter=x` to a file with an unchanged size and a newer modification time
- **WHEN** that file's status is read
- **THEN** the filter's command does not run and the file counts as unchanged

#### Scenario: A filter from the user's config
- **GIVEN** the user's global config defines a filter and the repository's config defines none
- **WHEN** a changed file's status is read
- **THEN** it counts as changed

### Requirement: The branch follows the repository
The editor SHALL watch the `HEAD` and `index` of every visited file's repository and read the branch and the file's status again when either changes, when the file changes, or when a rescan is reported, whatever the project's `watch` setting.

#### Scenario: A checkout outside the editor
- **WHEN** a visited file's repository is checked out to another branch by another program
- **THEN** the buffer's branch becomes the new one without a key press

#### Scenario: A commit outside the editor
- **WHEN** another program commits a changed file, rewriting `.git/index`
- **THEN** the file no longer counts as changed

#### Scenario: A save
- **WHEN** a clean file's buffer is edited and saved
- **THEN** the file counts as changed

#### Scenario: The rest of `.git` changes
- **WHEN** another program writes `.git/ORIG_HEAD`
- **THEN** the watcher reports nothing for it

#### Scenario: The project's index
- **WHEN** `HEAD` or `index` is reported
- **THEN** no project's index is walked again
