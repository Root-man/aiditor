# git-diff Specification

## Purpose
Answers one file's diff between two sides of a repository, re-diffs a buffer's text against a held old side, and lists the files that differ between two sides, so the diff view can draw them.

## Requirements

### Requirement: A file's text on two sides and its changed lines
Given a repository's top directory, a path relative to it and two sides, each a tree-ish, the index or the working tree, the file service SHALL answer the file's text on each side and its changed line ranges: runs of old-side lines and the new-side lines that replace them, in order, an empty range marking where lines are only added or only removed. Text SHALL be decoded as a buffer decodes a file: a byte-order mark stripped and CRLF read as LF. A file missing on a side SHALL be empty there. A file that is binary or not valid UTF-8 on either side SHALL answer a reason and no text. A tree-ish that names no tree SHALL answer git's error.

#### Scenario: Each pair of sides
- **GIVEN** a file committed twice, its next change staged, and a further change in the working tree
- **WHEN** it is diffed between the previous commit and `HEAD`, `HEAD` and the index, the index and the working tree, and `HEAD` and the working tree
- **THEN** each answers that pair's texts and the line they differ on

#### Scenario: Added, deleted and untracked
- **WHEN** a file added to the index is diffed `HEAD` → index, a file removed with `git rm` is diffed `HEAD` → index, and an untracked file is diffed index → working tree
- **THEN** the added file's old side is empty, the deleted file's new side is empty, and the untracked file's old side is empty, each with all its lines changed

#### Scenario: A binary file
- **WHEN** a committed file holding a NUL byte is diffed
- **THEN** the answer is a reason naming a binary file, and no text

#### Scenario: An unknown tree-ish
- **WHEN** a side names a tree-ish the repository doesn't have
- **THEN** the answer is git's error, not an empty side

### Requirement: A buffer's text re-diffed against a held old side
The file service SHALL answer the changed line ranges between a held old text and a snapshot of a buffer's text, worked out as for a file's diff, so unsaved edits are diffed as the buffer holds them.

#### Scenario: Same ranges as the saved file
- **WHEN** a held old text is diffed against a buffer snapshot, and the snapshot is then written to the working tree and the file is diffed from the side the old text came from
- **THEN** both answer the same ranges

### Requirement: The files that differ between two sides
Given a root and an old tree-ish, the file service SHALL list the files under the root that differ between it and a new tree-ish, or the working tree when none is given, as `git diff --name-status -z` reports them: each with its path relative to the repository's top directory, its change, and a rename's or copy's old path. Names SHALL be answered as they are on disk.

#### Scenario: Two tree-ishes
- **GIVEN** a commit that modifies, adds, deletes and renames files
- **WHEN** the files between its parent and it are listed
- **THEN** they are the files `git diff --name-status` reports, with the same changes, and the rename's old path

#### Scenario: A tree-ish and the working tree
- **WHEN** files are changed in the working tree, one of them with spaces and non-ASCII characters in its name, and the files between `HEAD` and the working tree are listed
- **THEN** each changed tracked file is listed, by its name as it is on disk

### Requirement: Snapshot trees outside the repository's objects
Both git operations SHALL accept an object directory and alternates for git to read objects from, so a tree written to another object directory can be a side.

#### Scenario: A snapshot tree
- **GIVEN** a tree written with `GIT_OBJECT_DIRECTORY` set to a separate directory and the repository's objects as its alternate
- **WHEN** a file is diffed and files are listed between that tree and the working tree, given those directories
- **THEN** both answer; without them, the diff answers git's error

### Requirement: Git runs nothing from the repository
The git operations SHALL run with no optional locks, literal pathspecs and `core.fsmonitor` off, SHALL refuse a tree-ish starting with `-`, and SHALL refuse to list against the working tree when the repository's own config defines a content filter, as the status operation does.

#### Scenario: A tree-ish that reads as an option
- **WHEN** a side's tree-ish is `--output=x`
- **THEN** the answer is an error and git is not run with it

### Requirement: A root's branches and tags
Given roots, the file service SHALL answer, for each, nothing when it is in no repository, else its repository's top directory and the short names of its local branches and tags, or git's error.

#### Scenario: Branches and tags
- **GIVEN** a repository with branches `main` and `topic` and a tag `v1`, and a directory outside git
- **WHEN** both are asked for
- **THEN** the repository answers its top directory and `main`, `topic` and `v1`, and the other directory answers nothing
