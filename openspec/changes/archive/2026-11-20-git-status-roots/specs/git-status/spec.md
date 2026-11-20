# Spec Delta

## Purpose
Answers, for each root of a project, its repository's branch and the files git reports changed there, so the diff view can list them.

## ADDED Requirements

### Requirement: Each root answers its branch and changed files
The file service SHALL answer, for each root it is given, the root of the repository holding it, its branch as the mode line names it, and every file under the root that git reports as changed or untracked, each with its path relative to the repository root, its staged state (`HEAD` → index), its unstaged state (index → working tree), and a rename's or copy's old path. Ignored and unchanged files SHALL NOT be reported. Roots SHALL run concurrently and the answer SHALL keep their order.

#### Scenario: Each kind of change
- **GIVEN** a repository with one file modified, one added to the index, one deleted, one renamed with `git mv` and one untracked
- **WHEN** its status is asked
- **THEN** the modified file is unstaged modified, the added file staged added, the deleted file unstaged deleted, the renamed file staged renamed with its old path, and the untracked file unstaged untracked

#### Scenario: Staged and changed again
- **WHEN** a file's change is staged and the file is then changed again
- **THEN** it is reported once, staged modified and unstaged modified

#### Scenario: Names as they are on disk
- **WHEN** an untracked file's name has spaces and non-ASCII characters
- **THEN** its reported path is that name, unquoted

#### Scenario: The branch
- **WHEN** the repository's `HEAD` is `ref: refs/heads/main`
- **THEN** the branch is `main`

#### Scenario: A root below its repository's top
- **WHEN** the root is a subdirectory of a repository that has changes outside it
- **THEN** only the changes under the root are reported, with paths relative to the repository root

### Requirement: A root outside git says so
A root with no repository at or above it SHALL answer that it is not a repository, and a root git fails on SHALL answer the error; neither SHALL keep the other roots from answering.

#### Scenario: A non-git root beside a repository
- **WHEN** the status of a plain directory and a repository is asked together
- **THEN** the directory answers that it isn't a repository and the repository answers its changes

### Requirement: Asking leaves the repository alone
Asking for status SHALL NOT write to the repository or run a command from its own config. A repository whose own config defines a content filter SHALL answer an error instead of running it.

#### Scenario: The index is left alone
- **WHEN** a repository's status is asked
- **THEN** `.git/index` is unchanged

#### Scenario: A repository's own filter
- **WHEN** a repository's `.git/config` defines `filter.x.clean`
- **THEN** its status answers an error and the filter's command does not run
