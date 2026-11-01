# Spec Delta

## MODIFIED Requirements

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
