# git-staging Specification

## Purpose
Stages, unstages, restores and deletes files of one repository for the diff view, answering git's error when one fails.

## Requirements

### Requirement: Steps run in order and stop at the first failure
The file service SHALL take a repository and a list of steps, each a kind and paths relative to the repository root, and run them in order: staging SHALL make the index match the working tree for those paths, deleted and untracked files included; unstaging SHALL make the index match `HEAD` for them, an unborn branch's index dropping them; restoring SHALL make the working tree match the index for them; deleting SHALL remove those files from the working tree. Paths SHALL be taken literally, not as patterns. The first step that fails SHALL stop the rest and answer git's message, or the file's error for a delete; otherwise it SHALL answer success.

#### Scenario: Staging a modified, a deleted and an untracked file
- **WHEN** a modified, a deleted and an untracked file are staged
- **THEN** `git diff --cached --name-status` shows them modified, deleted and added, and `git diff` shows nothing for them

#### Scenario: Unstaging a rename
- **GIVEN** a file renamed with `git mv`
- **WHEN** its new and old paths are unstaged
- **THEN** git reports the old path deleted and the new one untracked, and nothing staged

#### Scenario: Unstaging on an unborn branch
- **WHEN** a file added in a repository with no commits is unstaged
- **THEN** it is untracked

#### Scenario: Restoring and deleting
- **WHEN** a modified and a deleted file are restored and an untracked file is deleted
- **THEN** the first two have the index's text on disk and the untracked file is gone

#### Scenario: A failing step stops the rest
- **WHEN** restoring a path git doesn't know is followed by deleting a file
- **THEN** git's message is answered and the file is still there

### Requirement: Staging runs no command from the repository's config
A repository whose own config defines a content filter SHALL answer an error and run no step, since staging and restoring would run the filter.

#### Scenario: A repository's own filter
- **WHEN** a repository's `.git/config` defines `filter.x.clean` and a file is staged
- **THEN** an error is answered, the index is unchanged and the filter's command does not run

### Requirement: Applying a patch to the index
A step SHALL also be a unified patch of one file, applied to the index alone with the working tree left as it is; a patch that doesn't apply SHALL fail the step with git's message and leave the index unchanged. A patch SHALL carry the `\ No newline at end of file` marker where a side's last line has no newline.

#### Scenario: One change of several
- **GIVEN** a committed file with two changes in the working tree
- **WHEN** a patch holding the first change is applied
- **THEN** `git diff --cached` shows that change alone and `git diff` the other

#### Scenario: Changes at a file's start and end, without a final newline
- **GIVEN** a committed file without a final newline
- **WHEN** patches changing its first line and its last line are applied
- **THEN** the index holds both changes, its last line still without a newline

#### Scenario: A patch that doesn't apply
- **WHEN** a patch whose context isn't in the index is applied
- **THEN** git's message is answered and the index is unchanged
