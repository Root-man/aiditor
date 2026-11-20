# Spec Delta

## Purpose
Answers a tree id for a root's working tree, written outside the repository, so an agent turn's "before" side can be diffed against the working tree later.

## ADDED Requirements

### Requirement: A tree of the working tree as it is on disk
Given a root and a snapshot directory, the file service SHALL answer the root's repository top directory, the id of a tree holding every file of that repository's working tree as it is on disk, modified, staged and untracked files included and ignored files left out, and the object directory and alternates the tree can be read from. A root outside a git repository SHALL answer that it isn't one. A repository whose own config defines a content filter SHALL answer an error. A linked worktree or submodule SHALL be snapshotted from its own index and working tree.

#### Scenario: Every kind of change
- **GIVEN** a repository with a committed file modified on disk, a change staged and then changed again on disk, a deleted file, an untracked file and an ignored file
- **WHEN** its root is snapshotted
- **THEN** the tree, read with the answered objects, holds each file's text as it is on disk, no deleted file and no ignored file

#### Scenario: Not a repository
- **WHEN** a directory outside any repository is snapshotted
- **THEN** the answer says it isn't a repository

#### Scenario: A linked worktree
- **WHEN** a linked worktree with an untracked file is snapshotted
- **THEN** the tree holds the worktree's files, the untracked one included

### Requirement: The repository is untouched
A snapshot SHALL leave the repository's index, refs, object directory and working tree byte-for-byte unchanged, and SHALL write only inside the snapshot directory.

#### Scenario: Before and after
- **WHEN** a dirty repository is snapshotted
- **THEN** every file under the repository, `.git` included, holds the same bytes as before

### Requirement: An unchanged file is read once
The snapshot index SHALL be kept per repository in the snapshot directory and seed the next snapshot, so a file whose stat is unchanged since the last snapshot is not read again.

#### Scenario: A second snapshot of an unchanged dirty tree
- **GIVEN** a dirty tree snapshotted once
- **WHEN** nothing changes
- **THEN** the kept index's stat cache matches every file, so the next snapshot reads none, and it answers the same tree

### Requirement: Snapshot objects start empty at launch
Each editor process SHALL keep its snapshots in its own directory under the data dir, and at launch SHALL remove the directories of processes no longer running, leaving a running process's directory alone.

#### Scenario: A dead process's directory
- **GIVEN** a snapshot directory for a running process and one for a process that has exited
- **WHEN** the snapshot directories are cleared
- **THEN** only the exited process's directory is gone

### Requirement: Snapshot benchmark
The services benchmark SHALL time a snapshot of a tree with a few hundred changed files, first and repeated.

#### Scenario: Benchmark recorded
- **WHEN** the services benchmark runs
- **THEN** it reports the first and repeated snapshot times, and `docs/editor.md` records them
