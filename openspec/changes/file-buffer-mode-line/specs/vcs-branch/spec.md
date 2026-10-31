# Spec Delta

## Purpose

Tells a file buffer which branch its repository is on and keeps that current when the repository changes outside the editor.

## ADDED Requirements

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

### Requirement: The branch follows the repository
The editor SHALL watch the `HEAD` of every visited file's repository and read the branch again when it changes or a rescan is reported, whatever the project's `watch` setting.

#### Scenario: A checkout outside the editor
- **WHEN** a visited file's repository is checked out to another branch by another program
- **THEN** the buffer's branch becomes the new one without a key press

#### Scenario: The rest of `.git` changes
- **WHEN** another program commits, changing `.git/index`
- **THEN** the watcher reports nothing for it

#### Scenario: The project's index
- **WHEN** `HEAD` is reported
- **THEN** no project's index is walked again
